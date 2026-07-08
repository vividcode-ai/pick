use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use pick_agent::core::agent_loop::AgentLoopConfig;
use pick_agent::core::hooks::ToolEventBus;
use pick_agent::extensions::types::{
    ExtensionEvent, SessionBeforeCompactEvent, SessionCompactEvent,
};
use pick_agent::session::{CompactionEntry, SessionEntry, SessionEntryKind};
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
    if ctx.system_notifications_enabled.load(Ordering::Relaxed) {
        tool_event_bus.subscribe(Arc::new(crate::notification::SystemNotificationObserver));
    }

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
        Arc::new(move |_msg: &pick_ai::types::AssistantMessage| {
            goal_manager.budget_limit_reported()
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
                    let msg_text =
                        pick_agent::templates::render_objective_updated(&goal, &goal_manager);
                    msgs.push(Message::User(UserMessage::text(msg_text)));
                }

                match goal.status.as_str() {
                    "active" => {
                        let msg_text =
                            pick_agent::templates::render_steering_active(&goal, &goal_manager);
                        msgs.push(Message::User(UserMessage::text(msg_text)));
                    }
                    "budget_limited" if !goal_manager.mark_budget_limit_reported() => {
                        let msg_text = pick_agent::templates::render_steering_budget_limit(&goal);
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
        let interrupted = ctx.was_interrupted.clone();
        // Capture follow-up queue for queue draining
        let follow_up_queue = ctx.follow_up_queue.clone();
        // Capture cmd_tx for real-time notification when queued messages are consumed
        let follow_up_cmd_tx = cmd_tx.clone();
        move |_result: &pick_agent::core::agent_loop::AgentRunResult| {
            let mut msgs = Vec::new();

            // ESC interruption → auto-pause the goal
            if interrupted.swap(false, Ordering::Relaxed) {
                if let Err(e) = goal_manager.pause_on_interrupt() {
                    tracing::warn!("Failed to auto-pause goal on interrupt: {}", e);
                }
            }

            // Goal-driven continuation — only when no pending user input
            // (user messages in follow_up_queue take priority)
            let has_pending_user_input = follow_up_queue
                .lock()
                .map(|q| !q.is_empty())
                .unwrap_or(false);

            if !has_pending_user_input && goal_manager.can_continue() {
                if let Some(goal) = goal_manager.get()
                    && goal.status == "active"
                {
                    let _ = goal_manager.register_continuation();
                    let msg_text =
                        pick_agent::templates::render_follow_up_continuation(&goal, &goal_manager);
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

    let approve: Option<pick_agent::core::state::ApproveFn> = Some(Arc::new({
        let cmd_tx_clone = cmd_tx.clone();
        move |title: String, message: String| {
            let cmd_tx = cmd_tx_clone.clone();
            Box::pin(async move {
                let (tx, rx) = tokio::sync::oneshot::channel();
                let _ = cmd_tx.send(TuiCommand::ToolConfirm {
                    title,
                    message,
                    response_tx: tx,
                });
                match rx.await {
                    Ok(Ok(answers)) => answers
                        .first()
                        .and_then(|a| a.first())
                        .map(|s| s == "Allow")
                        .unwrap_or(false),
                    _ => false,
                }
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>
        }
    }));

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
        let last_goal_accounting = Arc::new(std::sync::Mutex::new(Instant::now()));
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
            let accounting = last_goal_accounting.clone();
            Box::pin(async move {
                // Wall-clock time accounting: compute delta since last accounting
                let now = Instant::now();
                let elapsed_secs = {
                    let mut last = accounting.lock().unwrap_or_else(|e| e.into_inner());
                    let delta = now.duration_since(*last).as_secs() as i64;
                    *last = now;
                    delta
                };
                if elapsed_secs > 0 {
                    gm.add_time_usage(elapsed_secs);
                }
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

    let sm = crate::core::settings::SettingsManager::load(&ctx.cwd);
    let enable_skills = sm.get_enable_skill_commands();
    let skill_paths: Vec<std::path::PathBuf> = if enable_skills {
        ctx.resource_loader
            .skills()
            .iter()
            .map(|s| s.file_path.clone())
            .collect()
    } else {
        Vec::new()
    };

    AgentLoopConfig {
        model: ctx.model.clone(),
        system_prompt: ctx.system_prompt.clone(),
        developer_sections: vec![],
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
        approve,
        skill_paths,
        parent_goal_manager: None,
    }
}

/// Auto-generate session title from the first user message
pub(crate) fn spawn_title_generation(
    title_text: String,
    cmd_tx: mpsc::UnboundedSender<TuiCommand>,
    model: pick_ai::types::Model,
    api_key: Option<String>,
) {
    tokio::spawn(async move {
        let send_title = |t: String| {
            let _ = cmd_tx.send(TuiCommand::SetSessionTitle(t));
        };

        let title = pick_agent::session::title::generate_title(&title_text, &model, api_key).await;

        if let Some(t) = title {
            send_title(t);
        }
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
