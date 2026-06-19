//! Agent loop runner - the core execution loop

use pick_ai::types::{
    AssistantMessage, ContentBlock, Context, Message, ToolCall, ToolDefinition, Usage,
};

use super::super::events::AgentEvent;
use super::super::state::{AgentState, ToolContext, ToolExecutionMode};
use super::stream::call_llm;
use super::tools::validate_tool_arguments;
use super::{
    AgentLoopConfig, AgentRunResult, MAX_CONSECUTIVE_TOOL_ERRORS, PLAN_RECOVERY_THRESHOLD,
};
use crate::extensions::types::{
    AgentEndEvent, AgentStartEvent, BeforeAgentStartEvent, ExtensionEvent, MessageEndEvent,
    ToolCallEvent, ToolResultEvent, TurnEndEvent, TurnStartEvent,
};
use crate::permission::guardian::GuardianAction;
use crate::permission::{Action, Ruleset};

// ===== Turn Control Flow =====

enum ContinueTurn {
    Continue,
    Break,
}

// ===== Helper functions =====

fn check_token_limit(config: &AgentLoopConfig, state: &mut AgentState, accumulated_usage: &Usage) {
    if let Some(ref follow_up_hook) = config.get_follow_up_messages {
        let follow_up_msgs = follow_up_hook(&AgentRunResult {
            messages: state.messages.clone(),
            usage: accumulated_usage.clone(),
        });
        if !follow_up_msgs.is_empty() {
            state.messages.extend(follow_up_msgs);
        }
    }
}

fn setup_initial_state(
    config: &AgentLoopConfig,
    initial_messages: Vec<Message>,
) -> (
    AgentState,
    Usage,
    usize,
    std::sync::Arc<tokio::sync::watch::Receiver<bool>>,
) {
    let tools = config.tools.clone();
    let state = AgentState {
        system_prompt: config.system_prompt.clone(),
        model: config.model.clone(),
        thinking_level: config.thinking_level,
        tools,
        messages: initial_messages,
        is_streaming: false,
        pending_tool_calls: Vec::new(),
        error_message: None,
        consecutive_tool_errors: 0,
        plan_awareness_triggered: false,
    };

    let accumulated_usage = Usage::zero();
    let turn_index: usize = 0;

    let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    let cancel_rx = std::sync::Arc::new(cancel_rx);

    if let Some(ref handler) = config.on_event {
        handler(AgentEvent::AgentStart);
    }
    if let Some(ref ext) = config.extension_runner {
        ext.emit(&ExtensionEvent::AgentStart(AgentStartEvent));
    }

    (state, accumulated_usage, turn_index, cancel_rx)
}

fn prepare_continue_state(
    config: &AgentLoopConfig,
    existing_messages: Vec<Message>,
) -> (
    AgentState,
    Usage,
    usize,
    std::sync::Arc<tokio::sync::watch::Receiver<bool>>,
) {
    let tools = config.tools.clone();
    let state = AgentState {
        system_prompt: config.system_prompt.clone(),
        model: config.model.clone(),
        thinking_level: config.thinking_level,
        tools,
        messages: existing_messages,
        is_streaming: false,
        pending_tool_calls: Vec::new(),
        error_message: None,
        consecutive_tool_errors: 0,
        plan_awareness_triggered: false,
    };

    let accumulated_usage = Usage::zero();
    let turn_index: usize = 0;

    let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    let cancel_rx = std::sync::Arc::new(cancel_rx);

    (state, accumulated_usage, turn_index, cancel_rx)
}

async fn process_llm_stream(
    config: &AgentLoopConfig,
    state: &mut AgentState,
    accumulated_usage: &mut Usage,
    cancel_rx: std::sync::Arc<tokio::sync::watch::Receiver<bool>>,
) -> Result<(AssistantMessage, Vec<ToolCall>), String> {
    if let Some(ref steering_hook) = config.get_steering_messages {
        let steering_msgs = steering_hook();
        if !steering_msgs.is_empty() {
            state.messages.extend(steering_msgs);
        }
    }

    let tools_defs: Vec<ToolDefinition> = state
        .tools
        .iter()
        .map(|t| ToolDefinition::new(&t.name, &t.description, t.parameters.clone()))
        .collect();

    let mut context = Context {
        system_prompt: Some(state.system_prompt.clone()),
        messages: state.messages.clone(),
        tools: Some(tools_defs),
    };

    if let Some(ref transform) = config.transform_context {
        context = transform(context);
    }

    let api_key_override = config.get_api_key.as_ref().and_then(|f| f());

    let assistant_msg = call_llm(
        &state.model,
        context,
        config.on_event.as_ref(),
        Some(cancel_rx),
        state.thinking_level,
        api_key_override,
        config.provider_max_retries,
        config.provider_max_retry_delay_ms,
    )
    .await?;

    accumulated_usage.input += assistant_msg.usage.input;
    accumulated_usage.output += assistant_msg.usage.output;
    accumulated_usage.cache_read += assistant_msg.usage.cache_read;
    accumulated_usage.cache_write += assistant_msg.usage.cache_write;

    let tool_calls: Vec<ToolCall> = assistant_msg
        .content
        .iter()
        .filter_map(|c| {
            if let ContentBlock::ToolCall(tc) = c {
                Some(tc.clone())
            } else {
                None
            }
        })
        .collect();

    Ok((assistant_msg, tool_calls))
}

async fn handle_tool_execution(
    config: &AgentLoopConfig,
    state: &mut AgentState,
    tool_calls: &[ToolCall],
    cancel_rx: std::sync::Arc<tokio::sync::watch::Receiver<bool>>,
) -> (Vec<pick_ai::types::ToolResultMessage>, bool) {
    let mut tool_results: Vec<pick_ai::types::ToolResultMessage> = Vec::new();
    let mut all_terminate = true;

    // Separate tool calls by execution mode
    let mut sequential_calls = Vec::new();
    let mut parallel_calls = Vec::new();

    for tc in tool_calls {
        // Guardian circuit breaker: if too many consecutive denials, interrupt
        if let Some(ref pm) = config.permission_manager
            && pm.is_guardian_circuit_broken()
            && let Some(msg) = pm.guardian_circuit_message()
        {
            let error_msg = pick_ai::types::ToolResultMessage::new(
                &tc.id,
                &tc.name,
                vec![ContentBlock::text(format!("Error: {}", msg))],
                true,
            );
            state.messages.push(Message::ToolResult(error_msg.clone()));
            tool_results.push(error_msg);
            continue;
        }

        // Permission pre-tool-use hooks + permission request hooks
        if let Some(ref hook_registry) = config.permission_hooks {
            if hook_registry.has_pre_hooks()
                || hook_registry.has_permission_hooks()
                || config.mode_rulesets.is_some()
            {
                let pre_ctx = crate::permission::hooks::PreToolUseContext {
                    tool_name: tc.name.clone(),
                    tool_call_id: tc.id.clone(),
                    input: tc.arguments.clone(),
                };
                if let Some(reason) = hook_registry.run_pre_hooks(&pre_ctx) {
                    all_terminate = false;
                    if let Some(ref handler) = config.on_event {
                        handler(AgentEvent::ToolExecutionStart {
                            tool_call_id: tc.id.clone(),
                            tool_name: tc.name.clone(),
                            args: tc.arguments.clone(),
                        });
                        handler(AgentEvent::ToolExecutionEnd {
                            tool_call_id: tc.id.clone(),
                            tool_name: tc.name.clone(),
                            result: serde_json::json!({"error": reason}),
                            is_error: true,
                        });
                    }
                    let error_msg = pick_ai::types::ToolResultMessage::new(
                        &tc.id,
                        &tc.name,
                        vec![ContentBlock::text(format!("Error: {}", reason))],
                        true,
                    );
                    state.messages.push(Message::ToolResult(error_msg.clone()));
                    tool_results.push(error_msg);
                    // Record guardian denial
                    if let Some(ref pm) = config.permission_manager {
                        if let Some(ref guard) = pm.guardian {
                            guard.record_result(GuardianAction::Deny);
                        }
                        pm.audit(
                            &tc.name,
                            "pre_hook",
                            &tc.arguments.to_string(),
                            crate::permission::audit::AuditDecision::Deny,
                            crate::permission::audit::AuditLayer::PreHook,
                            &reason,
                            None,
                        );
                    }
                    continue;
                }

                // Mode ruleset check
                let mut ruleset_action: Option<Action> = None;
                if let Some(ref rulesets) = config.mode_rulesets {
                    let tool_args_str = crate::permission::evaluate::extract_tool_args(tc);
                    let ruleset_refs: Vec<&Ruleset> = rulesets.iter().collect();
                    ruleset_action = Some(crate::permission::evaluate::evaluate_tool(
                        &tc.name,
                        &tool_args_str,
                        &ruleset_refs,
                    ));
                }

                match ruleset_action {
                    Some(Action::Allow) => {}
                    Some(Action::Deny) => {
                        all_terminate = false;
                        if let Some(ref handler) = config.on_event {
                            handler(AgentEvent::ToolExecutionStart {
                                tool_call_id: tc.id.clone(),
                                tool_name: tc.name.clone(),
                                args: tc.arguments.clone(),
                            });
                            handler(AgentEvent::ToolExecutionEnd {
                                tool_call_id: tc.id.clone(),
                                tool_name: tc.name.clone(),
                                result: serde_json::json!({"error": "Permission denied by mode ruleset"}),
                                is_error: true,
                            });
                        }
                        let msg = "Error: Permission denied by mode ruleset";
                        let error_msg = pick_ai::types::ToolResultMessage::new(
                            &tc.id,
                            &tc.name,
                            vec![ContentBlock::text(msg)],
                            true,
                        );
                        state.messages.push(Message::ToolResult(error_msg.clone()));
                        tool_results.push(error_msg);
                        if let Some(ref pm) = config.permission_manager {
                            pm.audit(
                                &tc.name,
                                "mode_ruleset",
                                "",
                                crate::permission::audit::AuditDecision::Deny,
                                crate::permission::audit::AuditLayer::ModeRuleset,
                                msg,
                                None,
                            );
                        }
                        continue;
                    }
                    _ => {
                        if hook_registry.has_permission_hooks() {
                            let perm_ctx = crate::permission::hooks::PermissionRequestContext {
                                tool_name: tc.name.clone(),
                                tool_args: crate::permission::evaluate::extract_tool_args(tc),
                                permission: crate::permission::tool_to_permission_key(&tc.name)
                                    .to_string(),
                                reason: None,
                            };
                            match hook_registry.run_permission_hooks(&perm_ctx).await {
                                Some(true) => {
                                    if let Some(ref pm) = config.permission_manager {
                                        pm.audit(
                                            &tc.name,
                                            &perm_ctx.permission,
                                            &perm_ctx.tool_args,
                                            crate::permission::audit::AuditDecision::Allow,
                                            crate::permission::audit::AuditLayer::PermissionHook,
                                            "Approved by user",
                                            None,
                                        );
                                    }
                                }
                                Some(false) => {
                                    all_terminate = false;
                                    if let Some(ref handler) = config.on_event {
                                        handler(AgentEvent::ToolExecutionStart {
                                            tool_call_id: tc.id.clone(),
                                            tool_name: tc.name.clone(),
                                            args: tc.arguments.clone(),
                                        });
                                        handler(AgentEvent::ToolExecutionEnd {
                                            tool_call_id: tc.id.clone(),
                                            tool_name: tc.name.clone(),
                                            result: serde_json::json!({"error": "Blocked by permission policy"}),
                                            is_error: true,
                                        });
                                    }
                                    let msg = "Error: Tool call was denied by permission policy";
                                    let error_msg = pick_ai::types::ToolResultMessage::new(
                                        &tc.id,
                                        &tc.name,
                                        vec![ContentBlock::text(msg)],
                                        true,
                                    );
                                    state.messages.push(Message::ToolResult(error_msg.clone()));
                                    tool_results.push(error_msg);
                                    if let Some(ref pm) = config.permission_manager {
                                        if let Some(ref guard) = pm.guardian {
                                            guard.record_result(GuardianAction::Deny);
                                        }
                                        pm.audit(
                                            &tc.name,
                                            &perm_ctx.permission,
                                            &perm_ctx.tool_args,
                                            crate::permission::audit::AuditDecision::Deny,
                                            crate::permission::audit::AuditLayer::PermissionHook,
                                            "Denied by user",
                                            None,
                                        );
                                    }
                                    continue;
                                }
                                None => {}
                            }
                        }
                    }
                }
            }
        } else if let Some(ref rulesets) = config.mode_rulesets {
            // Standalone mode ruleset check (no permission hooks configured)
            let tool_args_str = crate::permission::evaluate::extract_tool_args(tc);
            let ruleset_refs: Vec<&Ruleset> = rulesets.iter().collect();
            let perm_key = crate::permission::tool_to_permission_key(&tc.name);
            match crate::permission::evaluate::evaluate_tool(
                &tc.name,
                &tool_args_str,
                &ruleset_refs,
            ) {
                crate::permission::Action::Allow => {}
                crate::permission::Action::Deny => {
                    all_terminate = false;
                    if let Some(ref handler) = config.on_event {
                        handler(AgentEvent::ToolExecutionStart {
                            tool_call_id: tc.id.clone(),
                            tool_name: tc.name.clone(),
                            args: tc.arguments.clone(),
                        });
                        handler(AgentEvent::ToolExecutionEnd {
                            tool_call_id: tc.id.clone(),
                            tool_name: tc.name.clone(),
                            result: serde_json::json!({"error": "Permission denied by mode ruleset"}),
                            is_error: true,
                        });
                    }
                    let msg = "Error: Permission denied by mode ruleset";
                    let error_msg = pick_ai::types::ToolResultMessage::new(
                        &tc.id,
                        &tc.name,
                        vec![ContentBlock::text(msg)],
                        true,
                    );
                    state.messages.push(Message::ToolResult(error_msg.clone()));
                    tool_results.push(error_msg);
                    if let Some(ref pm) = config.permission_manager {
                        pm.audit(
                            &tc.name,
                            perm_key,
                            &tool_args_str,
                            crate::permission::audit::AuditDecision::Deny,
                            crate::permission::audit::AuditLayer::ModeRuleset,
                            msg,
                            None,
                        );
                    }
                    continue;
                }
                crate::permission::Action::Ask => {
                    all_terminate = false;
                    let msg = "Error: Tool requires approval but no permission hooks configured";
                    if let Some(ref handler) = config.on_event {
                        handler(AgentEvent::ToolExecutionStart {
                            tool_call_id: tc.id.clone(),
                            tool_name: tc.name.clone(),
                            args: tc.arguments.clone(),
                        });
                        handler(AgentEvent::ToolExecutionEnd {
                            tool_call_id: tc.id.clone(),
                            tool_name: tc.name.clone(),
                            result: serde_json::json!({"error": msg}),
                            is_error: true,
                        });
                    }
                    let error_msg = pick_ai::types::ToolResultMessage::new(
                        &tc.id,
                        &tc.name,
                        vec![ContentBlock::text(msg)],
                        true,
                    );
                    state.messages.push(Message::ToolResult(error_msg.clone()));
                    tool_results.push(error_msg);
                    if let Some(ref pm) = config.permission_manager {
                        pm.audit(
                            &tc.name,
                            perm_key,
                            &tool_args_str,
                            crate::permission::audit::AuditDecision::Ask,
                            crate::permission::audit::AuditLayer::ModeRuleset,
                            msg,
                            None,
                        );
                    }
                    continue;
                }
            }
        }

        // before_tool_call hook
        if let Some(ref before_hook) = config.before_tool_call
            && let Some(error) = before_hook(tc)
        {
            all_terminate = false;
            if let Some(ref handler) = config.on_event {
                handler(AgentEvent::ToolExecutionStart {
                    tool_call_id: tc.id.clone(),
                    tool_name: tc.name.clone(),
                    args: tc.arguments.clone(),
                });
                handler(AgentEvent::ToolExecutionEnd {
                    tool_call_id: tc.id.clone(),
                    tool_name: tc.name.clone(),
                    result: serde_json::json!({"error": error}),
                    is_error: true,
                });
            }
            let error_msg = pick_ai::types::ToolResultMessage::new(
                &tc.id,
                &tc.name,
                vec![ContentBlock::text(format!("Error: {}", error))],
                true,
            );
            state.messages.push(Message::ToolResult(error_msg.clone()));
            tool_results.push(error_msg);
            continue;
        }

        match state.tools.iter().find(|t| t.name == tc.name) {
            Some(t) if t.execution_mode == ToolExecutionMode::Parallel => {
                parallel_calls.push((tc.clone(), t.clone()));
            }
            Some(_) => {
                sequential_calls.push(tc.clone());
            }
            None => {
                all_terminate = false;
                if let Some(ref handler) = config.on_event {
                    handler(AgentEvent::ToolExecutionStart {
                        tool_call_id: tc.id.clone(),
                        tool_name: tc.name.clone(),
                        args: tc.arguments.clone(),
                    });
                    handler(AgentEvent::ToolExecutionEnd {
                        tool_call_id: tc.id.clone(),
                        tool_name: tc.name.clone(),
                        result: serde_json::json!({"error": format!("Unknown tool: {}", tc.name)}),
                        is_error: true,
                    });
                }
                let error_msg = pick_ai::types::ToolResultMessage::new(
                    &tc.id,
                    &tc.name,
                    vec![ContentBlock::text(format!(
                        "Error: Unknown tool: {}",
                        tc.name
                    ))],
                    true,
                );
                state.messages.push(Message::ToolResult(error_msg.clone()));
                tool_results.push(error_msg);
            }
        }
    }

    // First, execute sequential tools one at a time
    // (Extension checks, validation, and progress forwarding are done inline)
    'seq: for tc in &sequential_calls {
        // Extension: check if tool call is blocked
        if let Some(ref ext) = config.extension_runner {
            let block_result = ext.emit_tool_call(&ToolCallEvent {
                tool_call_id: tc.id.clone(),
                tool_name: tc.name.clone(),
                input: tc.arguments.clone(),
            });
            if block_result.block {
                all_terminate = false;
                let reason = block_result
                    .reason
                    .unwrap_or_else(|| "Blocked by extension".to_string());
                if let Some(ref handler) = config.on_event {
                    handler(AgentEvent::ToolExecutionStart {
                        tool_call_id: tc.id.clone(),
                        tool_name: tc.name.clone(),
                        args: tc.arguments.clone(),
                    });
                    handler(AgentEvent::ToolExecutionEnd {
                        tool_call_id: tc.id.clone(),
                        tool_name: tc.name.clone(),
                        result: serde_json::json!({"error": reason}),
                        is_error: true,
                    });
                }

                let error_msg = pick_ai::types::ToolResultMessage::new(
                    &tc.id,
                    &tc.name,
                    vec![ContentBlock::text(format!("Error: {}", reason))],
                    true,
                );
                state.messages.push(Message::ToolResult(error_msg.clone()));
                tool_results.push(error_msg);
                continue;
            }
        }

        if let Some(ref handler) = config.on_event {
            handler(AgentEvent::ToolExecutionStart {
                tool_call_id: tc.id.clone(),
                tool_name: tc.name.clone(),
                args: tc.arguments.clone(),
            });
        }

        // Find the tool and set up progress channel
        let tool = state.tools.iter().find(|t| t.name == tc.name);

        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let tool_ctx = ToolContext {
            cancel: Some((*cancel_rx).clone()),
            progress: Some(progress_tx),
            approve: config.approve.clone(),
            question: config.question.clone(),
            agent_id: config.agent_id.clone(),
            agent_registry: config.agent_registry.clone(),
            default_model: Some(config.model.clone()),
            fs_policy: config.fs_policy.clone(),
            cwd: config.cwd.clone(),
            permission_manager: config.permission_manager.clone(),
            sandbox: config.sandbox.clone(),
        };

        // Spawn a task to forward progress events while the tool executes
        let progress_handler = config.on_event.clone();
        let progress_tool_call_id = tc.id.clone();
        let progress_tool_name = tc.name.clone();
        let progress_args = tc.arguments.clone();
        tokio::spawn(async move {
            while let Some(partial) = progress_rx.recv().await {
                if let Some(ref handler) = progress_handler {
                    // Check if progress data contains todo items
                    let todo_value = serde_json::from_str::<serde_json::Value>(&partial)
                        .ok()
                        .and_then(|v| v.get("todos").cloned());

                    handler(AgentEvent::ToolExecutionUpdate {
                        tool_call_id: progress_tool_call_id.clone(),
                        tool_name: progress_tool_name.clone(),
                        args: progress_args.clone(),
                        partial_result: serde_json::json!({"content": [partial]}),
                    });

                    if let Some(todos) = todo_value {
                        handler(AgentEvent::TodoUpdated { todos });
                    }
                }
            }
        });

        let validated_args = match tool {
            Some(t) => match validate_tool_arguments(t, &tc.arguments, &tc.arguments) {
                Ok(v) => v,
                Err(e) => {
                    state.consecutive_tool_errors += 1;
                    all_terminate = false;
                    if let Some(ref handler) = config.on_event {
                        handler(AgentEvent::ToolExecutionEnd {
                            tool_call_id: tc.id.clone(),
                            tool_name: tc.name.clone(),
                            result: serde_json::json!({"error": e}),
                            is_error: true,
                        });
                    }
                    let error_msg = pick_ai::types::ToolResultMessage::new(
                        &tc.id,
                        &tc.name,
                        vec![ContentBlock::text(format!("Error: {}", e))],
                        true,
                    );
                    state.messages.push(Message::ToolResult(error_msg.clone()));
                    tool_results.push(error_msg);
                    continue 'seq;
                }
            },
            None => tc.arguments.clone(),
        };

        let result = if let Some(tool) = tool {
            let execute_fn = tool.execute.clone();
            let tool_call_id = tc.id.clone();
            let args = validated_args;
            let tool_name = tc.name.clone();
            match tokio::spawn(async move { execute_fn(tool_call_id, args, tool_ctx).await }).await
            {
                Ok(r) => r,
                Err(join_err) => {
                    let panic_msg = if join_err.is_panic() {
                        let panic_obj = join_err.into_panic();
                        if let Some(s) = panic_obj.downcast_ref::<String>() {
                            s.clone()
                        } else if let Some(s) = panic_obj.downcast_ref::<&str>() {
                            s.to_string()
                        } else {
                            format!("{:?}", panic_obj)
                        }
                    } else {
                        "task cancelled".to_string()
                    };
                    Err(format!("Tool '{}' panicked: {}", tool_name, panic_msg))
                }
            }
        } else {
            Err(format!("Unknown tool: {}", tc.name))
        };

        match result {
            Ok(tool_result) => {
                state.consecutive_tool_errors = 0;
                // Record guardian allow
                if let Some(ref pm) = config.permission_manager {
                    if let Some(ref guard) = pm.guardian {
                        guard.record_result(GuardianAction::Allow);
                    }
                    pm.audit(
                        &tc.name,
                        "tool",
                        "",
                        crate::permission::audit::AuditDecision::Allow,
                        crate::permission::audit::AuditLayer::ModeRuleset,
                        "Tool execution completed",
                        None,
                    );
                }
                if !tool_result.terminate {
                    all_terminate = false;
                }

                if let Some(ref handler) = config.on_event {
                    let result_texts: Vec<String> = tool_result
                        .content
                        .iter()
                        .filter_map(|c| match c {
                            ContentBlock::Text(t) => Some(t.text.clone()),
                            _ => None,
                        })
                        .collect();
                    handler(AgentEvent::ToolExecutionEnd {
                        tool_call_id: tc.id.clone(),
                        tool_name: tc.name.clone(),
                        result: {
                            let mut m = serde_json::json!({ "content": result_texts });
                            if tool_result.is_error
                                && let Some(err) = result_texts.first()
                            {
                                m["error"] = serde_json::json!(err);
                            }
                            m
                        },
                        is_error: tool_result.is_error,
                    });
                }

                // Extension: emit tool_result event
                if let Some(ref ext) = config.extension_runner {
                    let result_blocks: Vec<serde_json::Value> = tool_result
                        .content
                        .iter()
                        .filter_map(|c| serde_json::to_value(c).ok())
                        .collect();
                    ext.emit_tool_result(&ToolResultEvent {
                        tool_call_id: tc.id.clone(),
                        tool_name: tc.name.clone(),
                        input: tc.arguments.clone(),
                        content: result_blocks,
                        is_error: tool_result.is_error,
                    });
                }

                // PostToolUse hooks
                if let Some(ref hook_registry) = config.permission_hooks
                    && hook_registry.has_post_hooks()
                {
                    let post_ctx = crate::permission::hooks::PostToolUseContext {
                        tool_name: tc.name.clone(),
                        tool_call_id: tc.id.clone(),
                        input: tc.arguments.clone(),
                        output: serde_json::json!({"content": tool_result.content}),
                        is_error: tool_result.is_error,
                    };
                    hook_registry.run_post_hooks(&post_ctx);
                }

                let tool_result_msg = pick_ai::types::ToolResultMessage::new(
                    &tc.id,
                    &tc.name,
                    tool_result.content.clone(),
                    tool_result.is_error,
                );
                state
                    .messages
                    .push(Message::ToolResult(tool_result_msg.clone()));
                tool_results.push(tool_result_msg);
            }
            Err(e) => {
                state.consecutive_tool_errors += 1;
                all_terminate = false;
                if let Some(ref handler) = config.on_event {
                    handler(AgentEvent::ToolExecutionEnd {
                        tool_call_id: tc.id.clone(),
                        tool_name: tc.name.clone(),
                        result: serde_json::json!({"error": e}),
                        is_error: true,
                    });
                }

                // Extension: emit tool_result error
                if let Some(ref ext) = config.extension_runner {
                    ext.emit_tool_result(&ToolResultEvent {
                        tool_call_id: tc.id.clone(),
                        tool_name: tc.name.clone(),
                        input: tc.arguments.clone(),
                        content: vec![],
                        is_error: true,
                    });
                }

                let error_msg = pick_ai::types::ToolResultMessage::new(
                    &tc.id,
                    &tc.name,
                    vec![ContentBlock::text(format!("Error: {}", e))],
                    true,
                );
                state.messages.push(Message::ToolResult(error_msg.clone()));
                tool_results.push(error_msg);
            }
        }
    }

    // Execute parallel tools concurrently
    if !parallel_calls.is_empty() {
        let mut parallel_handles = Vec::new();
        let mut parallel_tool_infos = Vec::new();
        let approve = config.approve.clone();
        let question = config.question.clone();
        let agent_id = config.agent_id.clone();
        let agent_registry = config.agent_registry.clone();
        let default_model = config.model.clone();
        for (tc, tool) in parallel_calls {
            let cancel_rx_clone = cancel_rx.clone();
            let approve = approve.clone();
            let question = question.clone();
            let agent_id = agent_id.clone();
            let agent_registry = agent_registry.clone();
            let default_model = default_model.clone();
            let fs_policy = config.fs_policy.clone();
            let cwd = config.cwd.clone();
            let permission_manager = config.permission_manager.clone();
            parallel_tool_infos.push((tc.name.clone(), tc.id.clone()));
            parallel_handles.push(tokio::spawn(async move {
                let (progress_tx, _progress_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
                let tool_ctx = ToolContext {
                    cancel: Some((*cancel_rx_clone).clone()),
                    progress: Some(progress_tx),
                    approve,
                    question,
                    agent_id: agent_id.clone(),
                    agent_registry: agent_registry.clone(),
                    default_model: Some(default_model.clone()),
                    fs_policy: fs_policy.clone(),
                    cwd: cwd.clone(),
                    permission_manager: permission_manager.clone(),
                    sandbox: None,
                };
                let validated_args =
                    match validate_tool_arguments(&tool, &tc.arguments, &tc.arguments) {
                        Ok(v) => v,
                        Err(e) => return (tc, Err(e)),
                    };
                let result = (tool.execute)(tc.id.clone(), validated_args, tool_ctx).await;
                (tc, result)
            }));
        }

        for (handle, (tool_name, tool_id)) in parallel_handles.into_iter().zip(parallel_tool_infos)
        {
            match handle.await {
                Ok((tc, Ok(tool_result))) => {
                    state.consecutive_tool_errors = 0;
                    if !tool_result.terminate {
                        all_terminate = false;
                    }
                    if let Some(ref handler) = config.on_event {
                        let result_texts: Vec<String> = tool_result
                            .content
                            .iter()
                            .filter_map(|c| match c {
                                ContentBlock::Text(t) => Some(t.text.clone()),
                                _ => None,
                            })
                            .collect();
                        handler(AgentEvent::ToolExecutionEnd {
                            tool_call_id: tc.id.clone(),
                            tool_name: tc.name.clone(),
                            result: serde_json::json!({ "content": result_texts }),
                            is_error: tool_result.is_error,
                        });
                    }
                    if let Some(ref ext) = config.extension_runner {
                        let result_blocks: Vec<serde_json::Value> = tool_result
                            .content
                            .iter()
                            .filter_map(|c| serde_json::to_value(c).ok())
                            .collect();
                        ext.emit_tool_result(&ToolResultEvent {
                            tool_call_id: tc.id.clone(),
                            tool_name: tc.name.clone(),
                            input: tc.arguments.clone(),
                            content: result_blocks,
                            is_error: tool_result.is_error,
                        });
                    }
                    let tool_result_msg = pick_ai::types::ToolResultMessage::new(
                        &tc.id,
                        &tc.name,
                        tool_result.content.clone(),
                        tool_result.is_error,
                    );
                    state
                        .messages
                        .push(Message::ToolResult(tool_result_msg.clone()));
                    tool_results.push(tool_result_msg);
                }
                Ok((tc, Err(e))) => {
                    state.consecutive_tool_errors += 1;
                    all_terminate = false;
                    if let Some(ref handler) = config.on_event {
                        handler(AgentEvent::ToolExecutionEnd {
                            tool_call_id: tc.id.clone(),
                            tool_name: tc.name.clone(),
                            result: serde_json::json!({"error": e}),
                            is_error: true,
                        });
                    }
                    let error_msg = pick_ai::types::ToolResultMessage::new(
                        &tc.id,
                        &tc.name,
                        vec![ContentBlock::text(format!("Error: {}", e))],
                        true,
                    );
                    state.messages.push(Message::ToolResult(error_msg.clone()));
                    tool_results.push(error_msg);
                }
                Err(join_err) => {
                    all_terminate = false;
                    let panic_msg = if join_err.is_panic() {
                        let panic_obj = join_err.into_panic();
                        if let Some(s) = panic_obj.downcast_ref::<String>() {
                            s.clone()
                        } else if let Some(s) = panic_obj.downcast_ref::<&str>() {
                            s.to_string()
                        } else {
                            format!("{:?}", panic_obj)
                        }
                    } else {
                        "task cancelled".to_string()
                    };
                    let error_text = format!("Tool '{}' panicked: {}", tool_name, panic_msg);
                    if let Some(ref handler) = config.on_event {
                        handler(AgentEvent::ToolExecutionEnd {
                            tool_call_id: tool_id.clone(),
                            tool_name: tool_name.clone(),
                            result: serde_json::json!({"error": error_text}),
                            is_error: true,
                        });
                    }
                    let error_msg = pick_ai::types::ToolResultMessage::new(
                        &tool_id,
                        &tool_name,
                        vec![ContentBlock::text(format!("Error: {}", error_text))],
                        true,
                    );
                    state.messages.push(Message::ToolResult(error_msg.clone()));
                    tool_results.push(error_msg);
                }
            }
        }
    }

    (tool_results, all_terminate)
}

async fn execute_turn(
    config: &AgentLoopConfig,
    state: &mut AgentState,
    accumulated_usage: &mut Usage,
    turn_index: &mut usize,
    cancel_rx: std::sync::Arc<tokio::sync::watch::Receiver<bool>>,
) -> Result<ContinueTurn, String> {
    // Extension: before_agent_start
    if let Some(ref ext) = config.extension_runner {
        let bevent = BeforeAgentStartEvent {
            prompt: String::new(),
            system_prompt: state.system_prompt.clone(),
        };
        if let Some(result) = ext.emit_before_agent_start(&bevent)
            && let Some(ref sp) = result.system_prompt
        {
            state.system_prompt = sp.clone();
        }
    }

    if let Some(ref handler) = config.on_event {
        handler(AgentEvent::TurnStart);
    }
    if let Some(ref ext) = config.extension_runner {
        ext.emit(&ExtensionEvent::TurnStart(TurnStartEvent {
            turn_index: *turn_index,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }));
    }

    let (assistant_msg, tool_calls) =
        process_llm_stream(config, state, accumulated_usage, cancel_rx.clone()).await?;

    if tool_calls.is_empty() {
        state.consecutive_tool_errors = 0;
        if let Some(ref handler) = config.on_event {
            handler(AgentEvent::MessageStart {
                message: Message::Assistant(assistant_msg.clone()),
            });
        }

        state
            .messages
            .push(Message::Assistant(assistant_msg.clone()));

        if let Some(ref handler) = config.on_event {
            handler(AgentEvent::MessageEnd {
                message: Message::Assistant(assistant_msg.clone()),
            });
        }
        if let Some(ref ext) = config.extension_runner {
            ext.emit_message_end(&MessageEndEvent {
                message: serde_json::to_value(Message::Assistant(assistant_msg.clone()))
                    .unwrap_or_default(),
            });
        }

        if let Some(ref handler) = config.on_event {
            handler(AgentEvent::TurnEnd {
                message: Message::Assistant(assistant_msg),
                tool_results: vec![],
            });
        }
        if let Some(ref ext) = config.extension_runner {
            ext.emit(&ExtensionEvent::TurnEnd(TurnEndEvent {
                turn_index: *turn_index,
            }));
        }
        if let Some(ref persist) = config.on_turn_complete {
            persist(&state.messages).await;
        }

        return Ok(ContinueTurn::Break);
    }

    // Process tool calls
    if let Some(ref handler) = config.on_event {
        handler(AgentEvent::MessageStart {
            message: Message::Assistant(assistant_msg.clone()),
        });
    }
    state
        .messages
        .push(Message::Assistant(assistant_msg.clone()));

    if let Some(ref handler) = config.on_event {
        handler(AgentEvent::MessageEnd {
            message: Message::Assistant(assistant_msg.clone()),
        });
    }
    if let Some(ref ext) = config.extension_runner {
        ext.emit_message_end(&MessageEndEvent {
            message: serde_json::to_value(Message::Assistant(assistant_msg.clone()))
                .unwrap_or_default(),
        });
    }

    let (tool_results, all_terminate) =
        handle_tool_execution(config, state, &tool_calls, cancel_rx.clone()).await;

    // Plan-aware recovery: at moderate error count, suggest plan review
    if state.consecutive_tool_errors >= PLAN_RECOVERY_THRESHOLD && !state.plan_awareness_triggered {
        state.plan_awareness_triggered = true;
        let recovery_msg = format!(
            "[System] The last {} tool calls failed. If you were following a multi-step plan, \
             consider marking the current step as 'cancelled' or 'blocked' in the todo_plan \
             and proceeding to the next step. Review the todo_plan to see your remaining tasks.",
            state.consecutive_tool_errors
        );
        state
            .messages
            .push(Message::ToolResult(pick_ai::types::ToolResultMessage::new(
                "",
                "",
                vec![ContentBlock::text(recovery_msg)],
                false,
            )));
    }

    // If consecutive tool errors exceed hard threshold, force text-only mode
    if state.consecutive_tool_errors >= MAX_CONSECUTIVE_TOOL_ERRORS {
        let fallback_msg = format!(
            "[System] The agent has encountered {} consecutive tool errors. Switching to text-only mode. Please respond directly without using tools.",
            state.consecutive_tool_errors
        );
        state
            .messages
            .push(Message::ToolResult(pick_ai::types::ToolResultMessage::new(
                "",
                "",
                vec![ContentBlock::text(fallback_msg)],
                false,
            )));
        if let Some(ref handler) = config.on_event {
            handler(AgentEvent::TurnEnd {
                message: Message::Assistant(assistant_msg),
                tool_results,
            });
        }
        if let Some(ref ext) = config.extension_runner {
            ext.emit(&ExtensionEvent::TurnEnd(TurnEndEvent {
                turn_index: *turn_index,
            }));
        }
        if let Some(ref persist) = config.on_turn_complete {
            persist(&state.messages).await;
        }
        return Ok(ContinueTurn::Break);
    }

    if all_terminate {
        if let Some(ref handler) = config.on_event {
            handler(AgentEvent::TurnEnd {
                message: Message::Assistant(assistant_msg),
                tool_results,
            });
        }
        if let Some(ref ext) = config.extension_runner {
            ext.emit(&ExtensionEvent::TurnEnd(TurnEndEvent {
                turn_index: *turn_index,
            }));
        }
        if let Some(ref persist) = config.on_turn_complete {
            persist(&state.messages).await;
        }
        return Ok(ContinueTurn::Break);
    }

    let should_stop = config
        .should_stop_after_turn
        .as_ref()
        .and_then(|stop_hook| {
            if stop_hook(&assistant_msg) {
                Some(true)
            } else {
                None
            }
        })
        .is_some();

    if let Some(ref handler) = config.on_event {
        handler(AgentEvent::TurnEnd {
            message: Message::Assistant(assistant_msg),
            tool_results,
        });
    }
    if let Some(ref ext) = config.extension_runner {
        ext.emit(&ExtensionEvent::TurnEnd(TurnEndEvent {
            turn_index: *turn_index,
        }));
    }
    if let Some(ref persist) = config.on_turn_complete {
        persist(&state.messages).await;
    }
    *turn_index += 1;

    if should_stop {
        return Ok(ContinueTurn::Break);
    }

    Ok(ContinueTurn::Continue)
}

async fn execute_continue_turn(
    config: &AgentLoopConfig,
    state: &mut AgentState,
    accumulated_usage: &mut Usage,
    turn_index: &mut usize,
    cancel_rx: std::sync::Arc<tokio::sync::watch::Receiver<bool>>,
) -> Result<ContinueTurn, String> {
    if let Some(ref handler) = config.on_event {
        handler(AgentEvent::TurnStart);
    }
    if let Some(ref ext) = config.extension_runner {
        ext.emit(&ExtensionEvent::TurnStart(TurnStartEvent {
            turn_index: *turn_index,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }));
    }

    let (assistant_msg, tool_calls) =
        process_llm_stream(config, state, accumulated_usage, cancel_rx.clone()).await?;

    if tool_calls.is_empty() {
        state.consecutive_tool_errors = 0;
        if let Some(ref handler) = config.on_event {
            handler(AgentEvent::MessageStart {
                message: Message::Assistant(assistant_msg.clone()),
            });
        }

        state
            .messages
            .push(Message::Assistant(assistant_msg.clone()));

        if let Some(ref handler) = config.on_event {
            handler(AgentEvent::MessageEnd {
                message: Message::Assistant(assistant_msg.clone()),
            });
        }
        if let Some(ref ext) = config.extension_runner {
            ext.emit_message_end(&MessageEndEvent {
                message: serde_json::to_value(Message::Assistant(assistant_msg.clone()))
                    .unwrap_or_default(),
            });
        }

        if let Some(ref handler) = config.on_event {
            handler(AgentEvent::TurnEnd {
                message: Message::Assistant(assistant_msg),
                tool_results: vec![],
            });
        }
        if let Some(ref ext) = config.extension_runner {
            ext.emit(&ExtensionEvent::TurnEnd(TurnEndEvent {
                turn_index: *turn_index,
            }));
        }
        if let Some(ref persist) = config.on_turn_complete {
            persist(&state.messages).await;
        }

        return Ok(ContinueTurn::Break);
    }

    if let Some(ref handler) = config.on_event {
        handler(AgentEvent::MessageStart {
            message: Message::Assistant(assistant_msg.clone()),
        });
    }
    state
        .messages
        .push(Message::Assistant(assistant_msg.clone()));

    if let Some(ref handler) = config.on_event {
        handler(AgentEvent::MessageEnd {
            message: Message::Assistant(assistant_msg.clone()),
        });
    }
    if let Some(ref ext) = config.extension_runner {
        ext.emit_message_end(&MessageEndEvent {
            message: serde_json::to_value(Message::Assistant(assistant_msg.clone()))
                .unwrap_or_default(),
        });
    }

    let (tool_results, all_terminate) =
        handle_tool_execution(config, state, &tool_calls, cancel_rx.clone()).await;

    // Plan-aware recovery
    if state.consecutive_tool_errors >= PLAN_RECOVERY_THRESHOLD && !state.plan_awareness_triggered {
        state.plan_awareness_triggered = true;
        let recovery_msg = format!(
            "[System] The last {} tool calls failed. If you were following a multi-step plan, \
             consider marking the current step as 'cancelled' or 'blocked' in the todo_plan \
             and proceeding to the next step. Review the todo_plan to see your remaining tasks.",
            state.consecutive_tool_errors
        );
        state
            .messages
            .push(Message::ToolResult(pick_ai::types::ToolResultMessage::new(
                "",
                "",
                vec![ContentBlock::text(recovery_msg)],
                false,
            )));
    }

    // If consecutive tool errors exceed hard threshold, force text-only mode
    if state.consecutive_tool_errors >= MAX_CONSECUTIVE_TOOL_ERRORS {
        let fallback_msg = format!(
            "[System] The agent has encountered {} consecutive tool errors. Switching to text-only mode. Please respond directly without using tools.",
            state.consecutive_tool_errors
        );
        state
            .messages
            .push(Message::ToolResult(pick_ai::types::ToolResultMessage::new(
                "",
                "",
                vec![ContentBlock::text(fallback_msg)],
                false,
            )));
        if let Some(ref handler) = config.on_event {
            handler(AgentEvent::TurnEnd {
                message: Message::Assistant(assistant_msg),
                tool_results,
            });
        }
        if let Some(ref ext) = config.extension_runner {
            ext.emit(&ExtensionEvent::TurnEnd(TurnEndEvent {
                turn_index: *turn_index,
            }));
        }
        if let Some(ref persist) = config.on_turn_complete {
            persist(&state.messages).await;
        }
        return Ok(ContinueTurn::Break);
    }

    if all_terminate {
        if let Some(ref handler) = config.on_event {
            handler(AgentEvent::TurnEnd {
                message: Message::Assistant(assistant_msg),
                tool_results,
            });
        }
        if let Some(ref ext) = config.extension_runner {
            ext.emit(&ExtensionEvent::TurnEnd(TurnEndEvent {
                turn_index: *turn_index,
            }));
        }
        if let Some(ref persist) = config.on_turn_complete {
            persist(&state.messages).await;
        }
        return Ok(ContinueTurn::Break);
    }

    let should_stop = config
        .should_stop_after_turn
        .as_ref()
        .and_then(|stop_hook| {
            if stop_hook(&assistant_msg) {
                Some(true)
            } else {
                None
            }
        })
        .is_some();

    if let Some(ref handler) = config.on_event {
        handler(AgentEvent::TurnEnd {
            message: Message::Assistant(assistant_msg),
            tool_results,
        });
    }
    if let Some(ref ext) = config.extension_runner {
        ext.emit(&ExtensionEvent::TurnEnd(TurnEndEvent {
            turn_index: *turn_index,
        }));
    }
    if let Some(ref persist) = config.on_turn_complete {
        persist(&state.messages).await;
    }
    *turn_index += 1;

    if should_stop {
        return Ok(ContinueTurn::Break);
    }

    Ok(ContinueTurn::Continue)
}

// ===== Main agent loop =====

/// Run the agent loop
pub async fn run_agent_loop(
    config: AgentLoopConfig,
    initial_messages: Vec<Message>,
) -> Result<AgentRunResult, String> {
    let (mut state, mut accumulated_usage, mut turn_index, cancel_rx) =
        setup_initial_state(&config, initial_messages);

    loop {
        match execute_turn(
            &config,
            &mut state,
            &mut accumulated_usage,
            &mut turn_index,
            cancel_rx.clone(),
        )
        .await?
        {
            ContinueTurn::Break => break,
            ContinueTurn::Continue => {}
        }
    }

    check_token_limit(&config, &mut state, &accumulated_usage);

    // Emit end events
    if let Some(ref handler) = config.on_event {
        handler(AgentEvent::AgentEnd {
            messages: state.messages.clone(),
        });
    }
    if let Some(ref ext) = config.extension_runner {
        let messages_val: Vec<serde_json::Value> = state
            .messages
            .iter()
            .filter_map(|m| serde_json::to_value(m).ok())
            .collect();
        ext.emit(&ExtensionEvent::AgentEnd(AgentEndEvent {
            messages: messages_val,
        }));
    }

    Ok(AgentRunResult {
        messages: state.messages,
        usage: accumulated_usage,
    })
}

/// Continue an agent loop with additional messages (retry / continue support).
/// Unlike `run_agent_loop`, this does NOT re-emit AgentStart or run before_agent_start
/// extensions — it picks up from where the previous turn left off.
pub async fn run_agent_loop_continue(
    config: AgentLoopConfig,
    existing_messages: Vec<Message>,
) -> Result<AgentRunResult, String> {
    let (mut state, mut accumulated_usage, mut turn_index, cancel_rx) =
        prepare_continue_state(&config, existing_messages);

    loop {
        match execute_continue_turn(
            &config,
            &mut state,
            &mut accumulated_usage,
            &mut turn_index,
            cancel_rx.clone(),
        )
        .await?
        {
            ContinueTurn::Break => break,
            ContinueTurn::Continue => {}
        }
    }

    check_token_limit(&config, &mut state, &accumulated_usage);

    // Emit end events
    if let Some(ref handler) = config.on_event {
        handler(AgentEvent::AgentEnd {
            messages: state.messages.clone(),
        });
    }
    if let Some(ref ext) = config.extension_runner {
        let messages_val: Vec<serde_json::Value> = state
            .messages
            .iter()
            .filter_map(|m| serde_json::to_value(m).ok())
            .collect();
        ext.emit(&ExtensionEvent::AgentEnd(AgentEndEvent {
            messages: messages_val,
        }));
    }

    Ok(AgentRunResult {
        messages: state.messages,
        usage: accumulated_usage,
    })
}
