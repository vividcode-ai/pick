use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use pick_agent::core::agent_loop::AgentLoopConfig;
use pick_agent::core::hooks::ToolEventBus;
use pick_agent::extensions::types::{
    ExtensionEvent, SessionBeforeCompactEvent, SessionCompactEvent,
};
use pick_agent::session::{CompactionEntry, SessionEntry, SessionEntryKind};
use pick_ai::Context;
use pick_ai::types::{ContentBlock, Message, UserMessage};
use tokio::sync::mpsc;

use super::context::TuiContext;
use super::types::*;

/// Build the AgentLoopConfig for the agent
pub(crate) fn build_agent_config(
    ctx: &mut TuiContext,
    cmd_tx: mpsc::UnboundedSender<TuiCommand>,
) -> AgentLoopConfig {
    // Create the tool event bus and register the system notification observer
    let tool_event_bus = Arc::new(ToolEventBus::new());
    tool_event_bus.subscribe(Arc::new(crate::notification::SystemNotificationObserver));

    let mode_rules = ctx.agent_mode.ruleset();
    let was_interrupted = ctx.was_interrupted.clone();
    let permission_manager = ctx.permission_manager.clone();

    let before_tool_call = {
        let mode_rules_clone = mode_rules.clone();
        Arc::new(move |tc: &pick_ai::types::ToolCall| -> Option<String> {
            let tool_args_str =
                if let Some(cmd) = tc.arguments.get("command").and_then(|c| c.as_str()) {
                    cmd.to_string()
                } else if let Some(path) = tc.arguments.get("path").and_then(|p| p.as_str()) {
                    path.to_string()
                } else {
                    tc.arguments.to_string()
                };
            pick_agent::permission::evaluate::check_permission(
                &tc.name,
                &tool_args_str,
                &[&mode_rules_clone],
            )
            .err()
        })
    };

    let should_stop_after_turn = {
        let goal_manager = ctx.session_manager.goal_manager();
        let budget_injected = Arc::new(AtomicBool::new(false));
        Arc::new(move |_msg: &pick_ai::types::AssistantMessage| {
            budget_injected.load(Ordering::Relaxed)
                && goal_manager
                    .get()
                    .map(|g| g.status == "budget_limited")
                    .unwrap_or(false)
        })
    };

    let get_steering_messages = {
        let mode = ctx.agent_mode;
        let interrupted = was_interrupted.clone();
        let goal_manager = ctx.session_manager.goal_manager();
        let budget_injected = Arc::new(AtomicBool::new(false));
        // Capture steer queue for queue draining
        let steer_queue = ctx.steer_queue.clone();
        // Capture cmd_tx for real-time notification when queued messages are consumed
        let steer_cmd_tx = cmd_tx.clone();
        move || {
            let mut msgs = Vec::new();

            // Dynamic messages (interruption, plan mode, goal context)
            if interrupted.swap(false, Ordering::Relaxed) {
                msgs.push(Message::User(UserMessage::text(
                    "[System: The previous assistant response was interrupted. The previous context is no longer relevant. Please respond to the new message below.]",
                )));
            }
            match mode {
                crate::core::agent_mode::AgentMode::Plan => {
                    msgs.push(Message::User(UserMessage::text(
                        crate::core::agent_mode::PLAN_MODE_REMINDER,
                    )));
                }
                crate::core::agent_mode::AgentMode::Build => {}
            }
            if let Some(goal) = goal_manager.get() {
                // Check if objective was updated since last turn
                if goal_manager.take_objective_updated() {
                    let objective = escape_xml_text(&goal.objective);
                    let token_budget_str = goal
                        .token_budget
                        .map(|b| b.to_string())
                        .unwrap_or_else(|| "none".to_string());
                    let remaining_tokens = goal_manager
                        .remaining_tokens()
                        .map(|r| r.to_string())
                        .unwrap_or_else(|| "unbounded".to_string());
                    let msg_text = render_goal_template(
                        include_str!("../../templates/goals/objective_updated.md"),
                        &[
                            ("objective", &objective),
                            ("tokens_used", &goal.tokens_used.to_string()),
                            ("token_budget", &token_budget_str),
                            ("remaining_tokens", &remaining_tokens),
                        ],
                    );
                    msgs.push(Message::User(UserMessage::text(msg_text)));
                }

                match goal.status.as_str() {
                    "active" => {
                        let objective = escape_xml_text(&goal.objective);
                        let token_budget_str = goal
                            .token_budget
                            .map(|b| b.to_string())
                            .unwrap_or_else(|| "none".to_string());
                        let remaining_tokens = goal_manager
                            .remaining_tokens()
                            .map(|r| r.to_string())
                            .unwrap_or_else(|| "unbounded".to_string());
                        let msg_text = render_goal_template(
                            include_str!("../../templates/goals/steering_active.md"),
                            &[
                                ("objective", &objective),
                                ("tokens_used", &goal.tokens_used.to_string()),
                                ("token_budget", &token_budget_str),
                                ("remaining_tokens", &remaining_tokens),
                                ("time_used_seconds", &goal.time_used_seconds.to_string()),
                            ],
                        );
                        msgs.push(Message::User(UserMessage::text(msg_text)));
                    }
                    "budget_limited" if !budget_injected.swap(true, Ordering::Relaxed) => {
                        let objective = escape_xml_text(&goal.objective);
                        let token_budget_str = goal
                            .token_budget
                            .map(|b| b.to_string())
                            .unwrap_or_else(|| "none".to_string());
                        let msg_text = render_goal_template(
                            include_str!("../../templates/goals/steering_budget_limit.md"),
                            &[
                                ("objective", &objective),
                                ("tokens_used", &goal.tokens_used.to_string()),
                                ("token_budget", &token_budget_str),
                                ("time_used_seconds", &goal.time_used_seconds.to_string()),
                            ],
                        );
                        msgs.push(Message::User(UserMessage::text(msg_text)));
                    }
                    _ => {}
                }
            }

            // Drain from steer queue and notify TUI about consumed messages
            if let Ok(mut queue) = steer_queue.lock() {
                let queued = queue.drain();
                if !queued.is_empty() {
                    // Notify TUI about each consumed message for real-time rendering
                    for msg in &queued {
                        if let Message::User(u) = msg {
                            let text: String = u
                                .content
                                .iter()
                                .filter_map(|b| {
                                    if let ContentBlock::Text(t) = b {
                                        Some(t.text.clone())
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            if !text.is_empty() {
                                let _ = steer_cmd_tx
                                    .send(super::types::TuiCommand::SteeringMessageConsumed(text));
                            }
                        }
                    }
                    msgs.extend(queued);
                }
            }

            msgs
        }
    };

    let get_follow_up_messages = {
        let goal_manager = ctx.session_manager.goal_manager();
        // Capture follow-up queue for queue draining
        let follow_up_queue = ctx.follow_up_queue.clone();
        // Capture cmd_tx for real-time notification when queued messages are consumed
        let follow_up_cmd_tx = cmd_tx.clone();
        move |_result: &pick_agent::core::agent_loop::AgentRunResult| {
            let mut msgs = Vec::new();

            // Goal-driven continuation — only when no pending user input
            // (user messages in follow_up_queue take priority)
            let has_pending_user_input = follow_up_queue
                .lock()
                .map(|q| !q.is_empty())
                .unwrap_or(false);

            if !has_pending_user_input && goal_manager.can_continue() {
                if let Some(goal) = goal_manager.get()
                    && goal.status == "active"
                    && goal_manager.register_continuation()
                {
                    let objective = escape_xml_text(&goal.objective);
                    let token_budget_str = goal
                        .token_budget
                        .map(|b| b.to_string())
                        .unwrap_or_else(|| "none".to_string());
                    let remaining_tokens = goal_manager
                        .remaining_tokens()
                        .map(|r| r.to_string())
                        .unwrap_or_else(|| "unbounded".to_string());
                    let msg_text = render_goal_template(
                        include_str!("../../templates/goals/follow_up_continuation.md"),
                        &[
                            ("objective", &objective),
                            ("tokens_used", &goal.tokens_used.to_string()),
                            ("token_budget", &token_budget_str),
                            ("remaining_tokens", &remaining_tokens),
                        ],
                    );
                    msgs.push(Message::User(UserMessage::text(msg_text)));
                }
            }

            // Drain from follow-up queue and notify TUI about consumed messages
            if let Ok(mut queue) = follow_up_queue.lock() {
                let queued = queue.drain();
                if !queued.is_empty() {
                    // Notify TUI about each consumed message for real-time rendering
                    for msg in &queued {
                        if let Message::User(u) = msg {
                            let text: String = u
                                .content
                                .iter()
                                .filter_map(|b| {
                                    if let ContentBlock::Text(t) = b {
                                        Some(t.text.clone())
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            if !text.is_empty() {
                                let _ = follow_up_cmd_tx
                                    .send(super::types::TuiCommand::FollowUpMessageConsumed(text));
                            }
                        }
                    }
                    msgs.extend(queued);
                }
            }

            msgs
        }
    };

    let question: Arc<
        dyn Fn(
                Vec<pick_agent::core::state::QuestionPrompt>,
            ) -> std::pin::Pin<
                Box<
                    dyn std::future::Future<Output = Result<Vec<Vec<String>>, String>>
                        + Send
                        + 'static,
                >,
            > + Send
            + Sync,
    > = Arc::new({
        let cmd_tx_clone = cmd_tx.clone();
        move |questions: Vec<pick_agent::core::state::QuestionPrompt>| {
            let cmd_tx = cmd_tx_clone.clone();
            Box::pin(async move {
                let (tx, rx) = tokio::sync::oneshot::channel();
                let _ = cmd_tx.send(TuiCommand::ShowQuestions {
                    prompts: questions,
                    response_tx: tx,
                });
                rx.await.map_err(|e| format!("question cancelled: {}", e))?
            })
                as std::pin::Pin<
                    Box<
                        dyn std::future::Future<Output = Result<Vec<Vec<String>>, String>>
                            + Send
                            + 'static,
                    >,
                >
        }
    });

    let on_turn_complete: Arc<
        dyn Fn(
                &[Message],
            )
                -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>>
            + Send
            + Sync,
    > = Arc::new({
        let goal_manager = ctx.session_manager.goal_manager();
        let cmd_tx_clone = cmd_tx.clone();
        move |messages: &[Message]| {
            let tokens: i64 = messages
                .iter()
                .filter_map(|m| {
                    if let Message::Assistant(a) = m {
                        Some((a.usage.input + a.usage.output) as i64)
                    } else {
                        None
                    }
                })
                .sum();
            let gm = goal_manager.clone();
            let tx = cmd_tx_clone.clone();
            Box::pin(async move {
                if tokens > 0
                    && let Some(goal) = gm.add_token_usage(tokens)
                {
                    let _ = tx.send(TuiCommand::GoalUpdated(
                        serde_json::to_value(&goal).unwrap_or_default(),
                    ));
                }
            })
                as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>>
        }
    });

    AgentLoopConfig {
        model: ctx.model.clone(),
        system_prompt: ctx.system_prompt.clone(),
        tools: ctx.tools.clone(),
        thinking_level: ctx.thinking_level,
        max_tokens: None,
        temperature: None,
        extension_runner: ctx.extension_runner.clone(),
        transform_context: None,
        get_api_key: None,
        mode_rulesets: Some(vec![mode_rules.clone()]),
        before_tool_call: Some(before_tool_call),
        should_stop_after_turn: Some(should_stop_after_turn),
        get_steering_messages: Some(Arc::new(get_steering_messages)),
        get_follow_up_messages: Some(Arc::new(get_follow_up_messages)),
        question: Some(question),
        agent_id: None,
        agent_registry: Some(ctx.agent_registry.clone()),
        on_event: Some(ctx.on_event.clone()),
        tool_event_bus: Some(tool_event_bus.clone()),
        fs_policy: permission_manager.fs_policy(),
        cwd: Some(std::env::current_dir().unwrap_or_default()),
        permission_hooks: Some(permission_manager.hook_registry.clone()),
        permission_manager: Some(permission_manager.clone()),
        sandbox: ctx.platform_sandbox.clone(),
        sandbox_enabled: Some(ctx.sandbox_enabled.clone()),
        cancel_signal_tx: None,
        on_turn_complete: Some(on_turn_complete),
        provider_max_retries: None,
        provider_max_retry_delay_ms: None,
        approve: None,
    }
}

/// Auto-generate session title from the first user message
pub(crate) fn spawn_title_generation(
    title_text: String,
    cmd_tx: mpsc::UnboundedSender<TuiCommand>,
    model: pick_ai::types::Model,
    api_key: Option<String>,
    title_prompt: String,
) {
    tokio::spawn(async move {
        let send_title = |t: String| {
            let _ = cmd_tx.send(TuiCommand::SetSessionTitle(t));
        };

        async fn call_provider(
            mdl: &pick_ai::types::Model,
            api_key: Option<String>,
            ctx: Context,
        ) -> (String, Option<String>) {
            let registry = pick_ai::registry::global_registry();
            let provider = match registry.get(mdl.api.as_str()) {
                Some(p) => p,
                None => {
                    return (
                        String::new(),
                        Some(format!("no provider for {}", mdl.api.as_str())),
                    );
                }
            };
            let stream_options = pick_ai::StreamOptions {
                temperature: None,
                max_tokens: Some(100),
                api_key,
                transport: None,
                cache_retention: None,
                session_id: None,
                headers: None,
                timeout_ms: None,
                max_retries: Some(3),
                max_retry_delay_ms: None,
                thinking_budget: None,
                reasoning: None,
                metadata: None,
                signal: None,
            };
            let mut rx = (provider.stream)(mdl.clone(), ctx, Some(stream_options));
            let mut text = String::new();
            let mut error = None;
            while let Some(event) = rx.recv().await {
                match event {
                    pick_ai::StreamEvent::Done { message, .. } => {
                        for block in message.content {
                            if let ContentBlock::Text(t) = block {
                                text.push_str(&t.text);
                            }
                        }
                        break;
                    }
                    pick_ai::StreamEvent::Error { error: e, .. } => {
                        error = Some(
                            e.error_message
                                .unwrap_or_else(|| "unknown error".to_string()),
                        );
                        break;
                    }
                    _ => {}
                }
            }
            (text, error)
        }

        fn clean_title(text: &str) -> Option<String> {
            text.lines()
                .filter_map(|l| {
                    let trimmed = l.trim();
                    if trimmed.is_empty() || trimmed.starts_with('<') {
                        None
                    } else {
                        Some(
                            trimmed
                                .trim_matches('"')
                                .trim_matches('\'')
                                .trim()
                                .to_string(),
                        )
                    }
                })
                .next()
                .filter(|l| !l.is_empty())
                .map(|l| {
                    if l.len() > 100 {
                        crate::utils::truncate_utf8(&l, 97)
                    } else {
                        l
                    }
                })
        }

        // Attempt 1: system_prompt + same-language instruction
        let ctx1 = Context {
            system_prompt: Some(title_prompt.clone()),
            messages: vec![
                UserMessage::text(format!(
                    "Generate a title in the SAME LANGUAGE as the user message below. \
                 Only output the title, nothing else.\n\n{}",
                    title_text
                ))
                .into(),
            ],
            tools: None,
        };
        let (resp1, _err1) = call_provider(&model, api_key.clone(), ctx1).await;
        let mut title = clean_title(&resp1);

        // Attempt 2: if still no title, try user-message-only
        if title.is_none() {
            let ctx2 = Context {
                system_prompt: None,
                messages: vec![
                    UserMessage::text(format!(
                        "Generate a very short title in the SAME LANGUAGE as the user message. \
                     Max 40 characters, no quotes, no explanation.\n\n{}",
                        title_text
                    ))
                    .into(),
                ],
                tools: None,
            };
            let (resp2, _err2) = call_provider(&model, api_key, ctx2).await;
            title = clean_title(&resp2);
        }

        let final_title = title.unwrap_or_else(|| {
            if title_text.len() > 50 {
                crate::utils::truncate_utf8(&title_text, 47)
            } else {
                title_text
            }
        });

        send_title(final_title);
    });
}

/// Auto-compact session after agent result if needed
pub(crate) async fn auto_compact_session(ctx: &mut TuiContext) {
    use crate::core::compaction::compaction::{
        CompactionSettings, compact, prepare_compaction, should_compact,
    };
    let compact_settings = CompactionSettings::default();

    // Calculate total tokens from all_messages
    let total_tokens: u64 = ctx
        .all_messages
        .iter()
        .filter_map(|m| {
            if let Message::Assistant(a) = m {
                Some(a.usage.total_tokens)
            } else {
                None
            }
        })
        .sum();

    if !should_compact(
        total_tokens as usize,
        ctx.model.context_window as usize,
        &compact_settings,
    ) {
        return;
    }

    ctx.tui.chat.add_system_message(&format!(
        "Auto-compacting ({} tokens / {} window)...",
        total_tokens, ctx.model.context_window
    ));

    let path_entries: Vec<serde_json::Value> = ctx
        .all_messages
        .iter()
        .map(|msg| {
            let id = uuid::Uuid::now_v7().to_string();
            let message_val = match msg {
                Message::User(u) => serde_json::json!({"role": "user", "content": u.content}),
                Message::Assistant(a) => serde_json::json!({
                    "role": "assistant",
                    "content": a.content,
                    "stopReason": format!("{:?}", a.stop_reason),
                    "usage": {
                        "input": a.usage.input,
                        "output": a.usage.output,
                        "cacheRead": a.usage.cache_read,
                        "cacheWrite": a.usage.cache_write,
                        "totalTokens": a.usage.total_tokens,
                    }
                }),
                Message::ToolResult(t) => serde_json::json!({
                    "role": "toolResult",
                    "content": t.content,
                    "toolCallId": t.tool_call_id,
                    "toolName": t.tool_name,
                    "isError": t.is_error,
                }),
            };
            serde_json::json!({"id": id, "type": "message", "message": message_val})
        })
        .collect();

    if let Some(ref runner) = ctx.extension_runner {
        runner.emit(&ExtensionEvent::SessionBeforeCompact(
            SessionBeforeCompactEvent {
                preparation: serde_json::json!({}),
                branch_entries: path_entries.clone(),
                custom_instructions: None,
            },
        ));
    }

    let api_key = ctx
        .auth
        .get_api_key(&ctx.provider, true)
        .await
        .unwrap_or_default();
    if let Some(preparation) = prepare_compaction(&path_entries, &compact_settings) {
        match compact(&preparation, &ctx.model, &api_key, None, None, None).await {
            Ok(compaction_result) => {
                let summary = compaction_result.summary;
                let before = ctx.all_messages.len();
                ctx.all_messages = vec![
                    UserMessage::text(format!("[Compacted conversation summary]\n\n{}", summary))
                        .into(),
                ];
                ctx.tui.chat.add_system_message(&format!(
                    "Auto-compacted ({} msgs → 1, {} tokens before).",
                    before, compaction_result.tokens_before
                ));
                let compact_entry = SessionEntry {
                    id: uuid::Uuid::now_v7().to_string(),
                    parent_id: None,
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    kind: SessionEntryKind::Compaction(CompactionEntry {
                        summary: summary.clone(),
                        token_count: Some(compaction_result.tokens_before as u64),
                    }),
                };
                if let Err(e) = ctx.session_manager.append(compact_entry).await {
                    tracing::warn!("Failed to persist compaction entry: {}", e);
                }
                if let Some(ref runner) = ctx.extension_runner {
                    runner.emit(&ExtensionEvent::SessionCompact(SessionCompactEvent {
                        compaction_entry: serde_json::json!({
                            "summary": summary,
                            "tokensBefore": compaction_result.tokens_before,
                        }),
                        from_extension: false,
                    }));
                }
            }
            Err(e) => {
                ctx.tui
                    .show_error(&format!("Auto-compaction failed: {}", e.message));
            }
        }
    }
}

/// Render a goal template by replacing `{{ var }}` placeholders with values.
pub(crate) fn render_goal_template(template: &str, vars: &[(&str, &str)]) -> String {
    use std::collections::HashMap;
    let vars: HashMap<&str, &str> = vars.iter().copied().collect();
    let mut result = template.to_string();
    for (key, value) in &vars {
        let padded = format!("{{{{ {} }}}}", key);
        result = result.replace(&padded, value);
        let tight = format!("{{{{{}}}}}", key);
        result = result.replace(&tight, value);
    }
    result
}

/// Escape text for safe inclusion in XML-like tags (e.g. `<goal_context>`).
pub(crate) fn escape_xml_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
