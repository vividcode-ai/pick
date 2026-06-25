use std::sync::atomic::Ordering;

use pick_agent::core::agent_loop::AgentRunResult;
use pick_agent::session::SessionEntry;
use pick_ai::types::{ContentBlock, Message, UserMessage};
use pick_tui::app::TuiAction;

use crate::core::updates;

use super::actions_login;
use super::actions_mcp;
use super::actions_model;
use super::actions_session;
use super::actions_settings;
use super::agent_exec;
use super::cmd_dispatch;
use super::context::TuiContext;
use super::init;
use super::tree_summarize;
use super::types::*;

/// Dispatch a TuiAction to the appropriate handler
/// Returns true if the TUI should quit.
pub(crate) async fn dispatch_action(ctx: &mut TuiContext, action: TuiAction) -> bool {
    match action {
        TuiAction::Quit => return true,
        TuiAction::Interrupt => handle_interrupt(ctx).await,
        TuiAction::CycleModel => actions_model::handle_cycle_model(ctx).await,
        TuiAction::CycleModelBackward => actions_model::handle_cycle_model_backward(ctx).await,
        TuiAction::SelectModel => actions_model::handle_select_model(ctx),
        TuiAction::CycleThinking => actions_model::handle_cycle_thinking(ctx),
        TuiAction::CycleMode => actions_model::handle_cycle_mode(ctx).await,
        TuiAction::OpenTree => actions_session::handle_open_tree(ctx).await,
        TuiAction::ApiKeySubmit(key) => actions_login::handle_api_key_submit(ctx, &key).await,
        TuiAction::SelectionCancelled => {
            handle_selection_cancelled(ctx).await;
        }
        TuiAction::SelectionResult(_idx, val) => {
            handle_selection_result(ctx, &val).await;
        }
        TuiAction::Submit(text) => {
            handle_submit(ctx, text).await;
        }
        TuiAction::QueueMessage(text) => {
            handle_queue_message(ctx, text).await;
        }
        TuiAction::QueueFollowUp(text) => {
            handle_follow_up_message(ctx, text).await;
        }
        TuiAction::UpdateResponse(choice) => {
            match choice {
                pick_tui::app::UpdateChoice::UpdateNow => {
                    if let Some(action) = crate::core::update_action::get_update_action() {
                        ctx.pending_update = Some(action);
                    }
                    return true;
                }
                pick_tui::app::UpdateChoice::Skip => {
                    // Nothing to persist — prompt will show again on next startup
                }
                pick_tui::app::UpdateChoice::Dismiss => {
                    // Silence notifications for this version only
                    updates::dismiss_version();
                }
            }
        }
    }
    false
}

/// Handle Interrupt action — cooperative cancellation for running agent
async fn handle_interrupt(ctx: &mut TuiContext) {
    if !ctx.agent_is_running {
        return;
    }

    if ctx.agent_cancel_requested.load(Ordering::Relaxed) {
        // Second Esc: hard abort via AbortHandle
        if let Some(ref handle) = ctx.agent_abort_handle {
            handle.abort();
        }
        return;
    }

    // First Esc: cooperative cancellation
    if let Ok(mut queue) = ctx.steer_queue.lock() {
        queue.clear();
    }
    if let Ok(mut queue) = ctx.follow_up_queue.lock() {
        queue.clear();
    }
    ctx.tui.pending_follow_up_messages.clear();

    ctx.agent_cancel_requested.store(true, Ordering::Relaxed);
    if let Some(ref cancel_tx) = ctx.agent_cancel_tx {
        let _ = cancel_tx.send(true);
    }
}

/// Post-processing after agent finishes
pub(crate) async fn handle_agent_finished(
    ctx: &mut TuiContext,
    result: Result<AgentRunResult, String>,
    prev_len: usize,
    cancel_requested: bool,
) {
    ctx.agent_is_running = false;
    ctx.agent_cancel_tx = None;
    ctx.agent_abort_handle = None;

    ctx.tui.chat.discard_active_stream();

    match result {
        Ok(agent_result) => {
            // Persist all new messages from this run
            for msg in &agent_result.messages[prev_len..] {
                if let Err(e) = ctx.session_manager.append(SessionEntry::from(msg)).await {
                    ctx.tui
                        .show_error(&format!("Session persist failed: {}", e));
                }
                // Reconcile pending_user_messages
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
                    if !text.is_empty()
                        && ctx
                            .tui
                            .pending_user_messages
                            .front()
                            .map(|f| f == &text)
                            .unwrap_or(false)
                    {
                        ctx.tui.pending_user_messages.pop_front();
                    }
                    // Also reconcile pending_follow_up_messages
                    if !text.is_empty()
                        && ctx
                            .tui
                            .pending_follow_up_messages
                            .front()
                            .map(|f| f == &text)
                            .unwrap_or(false)
                    {
                        ctx.tui.pending_follow_up_messages.pop_front();
                    }
                }
            }
            ctx.all_messages = agent_result.messages;

            // Show usage in chat (reads agent_start_time, so do this before stopping timer)
            ctx.tui
                .show_usage(agent_result.usage.input, agent_result.usage.output);

            // Stop timer
            ctx.tui.stop_agent_timer();
            let pct = (agent_result.usage.total_tokens as f64) / (ctx.model.context_window as f64)
                * 100.0;
            ctx.tui
                .set_context_info(Some(pct), ctx.model.context_window);

            // Clear status — usage is already shown in chat via show_usage
            ctx.tui.set_status(None);

            // Transition to Input state BEFORE auto-compaction so that the
            // main loop is in the correct state if Ctrl+C reaches the OS-level
            // handler during the potentially long-running compact call.
            // (The duplicate finalize_turn at the function bottom is harmless.)
            ctx.tui.finalize_turn();

            // Render usage status before auto-compaction (which may take a while)
            let _ = ctx.tui.render_with_terminal(&mut ctx.terminal_manager);

            agent_exec::auto_compact_session(ctx).await;
        }
        Err(e) => {
            ctx.tui.set_status(None);
            let _ = ctx.tui.render_with_terminal(&mut ctx.terminal_manager);
            ctx.tui.show_error(&e);
        }
    }

    if cancel_requested {
        ctx.was_interrupted.store(true, Ordering::Relaxed);
        ctx.tui.chat.add_system_message("Interrupted.");
    }

    ctx.tui.finalize_turn();
}

/// Handle QueueMessage action — enqueue user message into steering queue
async fn handle_queue_message(ctx: &mut TuiContext, text: String) {
    if let Ok(mut queue) = ctx.steer_queue.lock() {
        queue.enqueue(Message::User(UserMessage::text(&text)));
        // Track in pending_user_messages for above-editor rendering
        if !ctx.tui.pending_user_messages.contains(&text) {
            ctx.tui.pending_user_messages.push_back(text.clone());
        }
        let len = queue.len();
        // Send QueueUpdate for UI feedback
        let _ = ctx.cmd_tx.send(TuiCommand::QueueUpdate {
            steer_len: len,
            follow_up_len: ctx.follow_up_queue.lock().map(|q| q.len()).unwrap_or(0),
            next_turn_len: ctx.next_turn_queue.lock().map(|q| q.len()).unwrap_or(0),
        });
    }
}

/// Handle QueueFollowUp action — enqueue user message into follow-up queue
async fn handle_follow_up_message(ctx: &mut TuiContext, text: String) {
    if let Ok(mut queue) = ctx.follow_up_queue.lock() {
        queue.enqueue(Message::User(UserMessage::text(&text)));
        // Track in pending_follow_up_messages for above-editor rendering
        if !ctx.tui.pending_follow_up_messages.contains(&text) {
            ctx.tui.pending_follow_up_messages.push_back(text.clone());
        }
        let len = queue.len();
        // Send QueueUpdate for UI feedback
        let _ = ctx.cmd_tx.send(TuiCommand::QueueUpdate {
            steer_len: ctx.steer_queue.lock().map(|q| q.len()).unwrap_or(0),
            follow_up_len: len,
            next_turn_len: ctx.next_turn_queue.lock().map(|q| q.len()).unwrap_or(0),
        });
    }
}

/// Handle SelectionResult by dispatching to the correct handler
async fn handle_selection_result(ctx: &mut TuiContext, val: &str) {
    let prev_cmd = ctx.pending_command.clone();
    match prev_cmd.as_deref() {
        Some("connect") => actions_login::handle_login_selection(ctx, val).await,
        Some("login-oauth") => actions_login::handle_oauth_login(ctx, val).await,
        Some("login-apikey") => actions_login::handle_apikey_login(ctx, val),
        Some("unconnect") => actions_login::handle_logout(ctx, val),
        Some("settings") => {
            actions_settings::handle_settings_selection(ctx, val).await;
        }
        Some("settings-models") => handle_settings_models_selection(ctx, val).await,
        Some("tree") => {
            if actions_session::handle_tree_selection(ctx, val)
                .await
                .is_some()
            {
                ctx.pending_command = None;
                ctx.tui.finalize_turn();
            } else {
                return; // pending_command kept for summarization
            }
        }
        Some(cmd) if cmd.starts_with("tree-summarize:") => {
            tree_summarize::handle_tree_summarize(ctx, val).await;
        }
        Some("model") => handle_model_selection(ctx, val).await,
        Some("scoped-models") => handle_scoped_models_selection(ctx, val),
        Some("fork") => {
            if let Ok(idx) = val.parse::<usize>() {
                actions_session::handle_fork(ctx, idx).await;
            }
        }
        Some("resume") => actions_session::handle_resume(ctx, val).await,
        Some("mcp") => {
            actions_mcp::handle_mcp_server_selected(ctx, val).await;
            return; // pending_command handled internally
        }
        Some(cmd) if cmd.starts_with("mcp-server:") => {
            actions_mcp::handle_mcp_server_action(ctx, val).await;
            return; // pending_command handled internally
        }
        Some(cmd) if cmd.starts_with("mcp-caps:") => {
            actions_mcp::handle_mcp_capabilities_selection(ctx, val).await;
            return; // pending_command handled internally
        }
        _ => {
            ctx.tui
                .chat
                .add_system_message(&format!("\x1b[36m→\x1b[0m \x1b[1m{}\x1b[0m", val));
        }
    }
    // Only clear pending_command if the handler didn't chain to a new one.
    // If a sub-selector is now showing (ctx.tui.selection is Some), the handler
    // chained to a deeper level — keep pending_command alive.
    if ctx.pending_command == prev_cmd && ctx.tui.selection.is_none() {
        ctx.pending_command = None;
    }
    ctx.tui.finalize_turn();
}

/// Handle SelectionCancelled — user pressed Esc on a SelectList.
/// Used for multi-level navigation (e.g. MCP manager back/up one level).
async fn handle_selection_cancelled(ctx: &mut TuiContext) {
    let prev_cmd = ctx.pending_command.clone();
    match prev_cmd.as_deref() {
        Some(cmd) if cmd.starts_with("mcp-server:") => {
            // Level 2 (server detail/actions) → go back to Level 1 (server list)
            // Extract just the server name
            actions_mcp::show_mcp_server_list(ctx).await;
            // pending_command is set by show_mcp_server_list
            return;
        }
        Some(cmd) if cmd.starts_with("mcp-caps:") => {
            // Level 3 (capabilities) → go back to Level 2 (server detail/actions)
            let name = cmd.strip_prefix("mcp-caps:").unwrap_or("");
            actions_mcp::show_mcp_server_detail(ctx, name).await;
            return;
        }
        _ => {
            // No multi-level navigation: just clear pending_command
            ctx.pending_command = None;
        }
    }
    ctx.tui.finalize_turn();
}

/// Handle model selection from SelectModel or /model command
async fn handle_model_selection(ctx: &mut TuiContext, val: &str) {
    let new_provider = if val.contains('/') {
        let parts: Vec<&str> = val.splitn(2, '/').collect();
        parts[0].to_string()
    } else {
        ctx.provider.clone()
    };
    let new_model_id = if val.contains('/') {
        val.split_once('/').map(|x| x.1).unwrap_or(val).to_string()
    } else {
        val.to_string()
    };

    let (new_model, resolved_provider) = init::update_model(&new_provider, &new_model_id);
    if new_model.id != ctx.model.id || resolved_provider != ctx.provider {
        ctx.model = new_model;
        ctx.model_id = new_model_id;
        ctx.provider = resolved_provider.clone();
        init::save_default_model(&ctx.provider, &ctx.model_id);
        ctx.system_prompt = init::rebuild_system_prompt(
            &ctx.tools,
            &ctx.resource_loader,
            &ctx.cwd,
            &ctx.provider,
            &ctx.model_id,
            ctx.args.system_prompt.as_deref(),
            &ctx.args.append_system_prompt,
            Some(&ctx.agent_mode),
        );
        init::update_api_key(&ctx.auth, &ctx.provider).await;
        ctx.tui.model_id = ctx.model_id.clone();
        ctx.tui.provider = ctx.provider.clone();
        ctx.tui.chat.add_system_message(&format!(
            "Switched to model: \x1b[1m{} ({})\x1b[0m",
            ctx.model_id, ctx.provider
        ));
    } else {
        ctx.tui.chat.add_system_message(&format!(
            "Already using model: \x1b[1m{}\x1b[0m",
            ctx.model_id
        ));
    }
}

/// Handle scoped-models selection (multi-select toggle)
fn handle_scoped_models_selection(ctx: &mut TuiContext, val: &str) {
    let model_id = val.to_string();
    if let Some(pos) = ctx.scoped_models.iter().position(|m| m == &model_id) {
        ctx.scoped_models.remove(pos);
    } else {
        ctx.scoped_models.push(model_id);
    }

    // Re-show for multi-select behavior
    use pick_tui::components::select::{SelectItem, SelectList};
    let models = pick_ai::models::get_models(&ctx.provider);
    if !models.is_empty() {
        let items: Vec<SelectItem> = models
            .iter()
            .map(|m| {
                let enabled = ctx.scoped_models.contains(&m.id);
                let label = if enabled {
                    format!("[x] {}", m.id)
                } else {
                    format!("[ ] {}", m.id)
                };
                SelectItem::new(label, m.id.clone()).with_description(if enabled {
                    "enabled"
                } else {
                    "disabled"
                })
            })
            .collect();
        let status = if ctx.scoped_models.is_empty() {
            "All models available (none scoped)"
        } else {
            &format!("{} model(s) scoped", ctx.scoped_models.len())
        };
        let select = SelectList::new(format!("Scoped Models ({})", status), items);
        ctx.tui.start_selection(select);
        ctx.pending_command = Some("scoped-models".to_string());
        ctx.tui.finalize_turn();
    }
}

/// Handle settings-models selection
async fn handle_settings_models_selection(ctx: &mut TuiContext, val: &str) {
    use crate::core::settings::{Settings, SettingsManager};
    let cwd_set = std::env::current_dir().unwrap_or_default();
    let mut sm = SettingsManager::load(&cwd_set);
    let (new_provider, new_model_id) = if val.contains('/') {
        let parts: Vec<&str> = val.splitn(2, '/').collect();
        (parts[0].to_string(), parts[1..].join("/"))
    } else {
        (ctx.provider.clone(), val.to_string())
    };
    let mut update = Settings::default();
    update.default_provider = Some(new_provider.clone());
    update.default_model = Some(new_model_id.clone());
    match sm.set_global(update) {
        Ok(()) => ctx.tui.chat.add_system_message(&format!(
            "Default model saved: \x1b[1m{} ({})\x1b[0m",
            new_model_id, new_provider
        )),
        Err(e) => ctx
            .tui
            .show_error(&format!("Failed to save model setting: {}", e)),
    }
}

/// Handle Submit action — the main message submission flow (flat EventStream model).
/// Sets up user message, spawns agent task + watcher, and returns immediately.
/// The main loop handles all events (agent streaming, keyboard, lifecycle) through cmd_rx.
async fn handle_submit(ctx: &mut TuiContext, user_text: String) {
    // Clear stale paste accumulator before processing input
    ctx.tui.paste_accumulator.clear();
    ctx.tui.last_paste_time = None;

    let trimmed = user_text.trim().to_string();
    let mut user_text = user_text;

    // Check for /skill:name expansion
    let mut is_skill_expanded = false;
    let mut skip_user_message = false;

    if trimmed.starts_with("/skill:") {
        match cmd_dispatch::expand_skill_command(ctx, &user_text) {
            Some(expanded) => {
                user_text = expanded;
                is_skill_expanded = true;
            }
            None => return,
        }
    } else if let Some(cmd_full) = trimmed.strip_prefix('/') {
        let cmd_name = cmd_full.split_whitespace().next().unwrap_or("");
        let args_after: Vec<&str> = cmd_full.split_whitespace().skip(1).collect();
        let args: Vec<String> = args_after.iter().map(|s| s.to_string()).collect();

        // Check if this is a known built-in slash command — built-in commands
        // always take priority over command templates (matching pi's behavior).
        let is_builtin = crate::core::slash_commands::BUILTIN_SLASH_COMMANDS
            .iter()
            .any(|c| c.name == cmd_name);

        if is_builtin {
            match cmd_dispatch::handle_slash_command(ctx, cmd_name, &args, &user_text).await {
                cmd_dispatch::SlashCommandResult::Quit => {
                    ctx.tui.finalize_turn();
                    return;
                }
                cmd_dispatch::SlashCommandResult::Consumed => {
                    ctx.tui.finalize_turn();
                    return;
                }
                cmd_dispatch::SlashCommandResult::ContinueSubmit => {
                    skip_user_message = true;
                }
            }
        } else {
            // Not a built-in command — try command template expansion (from .pick/commands/ .md files)
            let commands = ctx.resource_loader.commands();
            let expanded =
                crate::core::prompt_templates::expand_prompt_template(&user_text, commands);
            if expanded != user_text {
                // Command template matched — expand and send to agent
                user_text = expanded;
            }
            // No match either — text is sent to agent as-is (matches pi behavior)
        }
    }

    // ---- Run the agent (flat EventStream model) ----
    // Commands that start with '/' (like /skill:name, /goal <text>) keep
    // state as Input in submit_input() — correct for sync slash commands
    // but not for those that spawn an agent task. Transition to Streaming
    // here so the UI shows the working state, Ctrl+C triggers immediate
    // quit, and has_ever_streamed reflects that the session ran.
    if ctx.tui.state != pick_tui::app::AppState::Streaming {
        ctx.tui.state = pick_tui::app::AppState::Streaming;
        ctx.tui.has_ever_streamed = true;
        ctx.tui.update_terminal_title();
    }
    ctx.tui.start_agent_timer();
    let prev_len = ctx.all_messages.len();

    if !skip_user_message {
        // Drain nextTurnQueue (messages survive abort, prepended to next user prompt)
        if let Ok(mut queue) = ctx.next_turn_queue.lock() {
            let next_msgs = queue.drain();
            if !next_msgs.is_empty() {
                ctx.tui.chat.add_system_message(&format!(
                    "📤 {} queued message(s) delivered",
                    next_msgs.len()
                ));
                ctx.all_messages.extend(next_msgs);
            }
        }

        ctx.all_messages
            .push(Message::User(UserMessage::text(&user_text)));
        if !is_skill_expanded {
            ctx.tui.chat.add_user_message(&user_text);
        }
    }

    // Auto-title generation for first user message
    let needs_title = {
        let is_custom = ctx
            .tui
            .session_name
            .as_deref()
            .is_some_and(|n| !super::types::is_default_session_title(n));
        !is_custom
            && ctx
                .all_messages
                .iter()
                .filter(|m| matches!(m, Message::User(_)))
                .count()
                == 1
    };
    if needs_title {
        let title_text = if skip_user_message {
            ctx.session_manager
                .goal_manager()
                .get()
                .map(|g| g.objective)
                .unwrap_or_default()
        } else {
            user_text.clone()
        };
        let api_key = ctx
            .auth
            .get_api_key(ctx.model.provider.as_str(), true)
            .await;
        agent_exec::spawn_title_generation(
            title_text,
            ctx.cmd_tx.clone(),
            ctx.model.clone(),
            api_key,
            include_str!("../../title_prompt.txt").to_string(),
        );
    }

    // Update UI: show "Working..." status
    let _ = ctx.tui.render_with_terminal(&mut ctx.terminal_manager);
    ctx.tui.set_status(Some("Working..."));
    let _ = ctx.tui.render_with_terminal(&mut ctx.terminal_manager);

    // Refresh tools and build agent config
    ctx.tools = init::refilter_tools(
        &ctx.all_tools,
        &ctx.agent_mode,
        &ctx.session_manager,
        &ctx.mcp_manager,
        ctx.mcp_enabled.load(std::sync::atomic::Ordering::Relaxed),
    )
    .await;
    let mut config = agent_exec::build_agent_config(ctx, ctx.cmd_tx.clone());

    // Create cooperative cancellation channel
    let (cancel_tx, _cancel_rx) = tokio::sync::watch::channel(false);
    let cancel_tx = std::sync::Arc::new(cancel_tx);
    config.cancel_signal_tx = Some(cancel_tx.clone());

    // Register TUI approval hook
    let approval_hook = super::types::TuiApprovalHook {
        cmd_tx: ctx.cmd_tx.clone(),
    };
    ctx.permission_manager
        .register_permission_hook(std::sync::Arc::new(approval_hook));

    let msgs = ctx.all_messages.clone();

    // Spawn agent task
    let agent_handle = tokio::spawn(async move {
        crate::core::agent_session::run_agent_loop_with_retry_and_continuation(
            config,
            msgs,
            Default::default(),
            None,
        )
        .await
    });

    // Store abort handle for second-Esc hard abort
    let abort_handle = agent_handle.abort_handle();
    ctx.agent_abort_handle = Some(abort_handle);

    // Wire cancellation state into context (shared with main loop via AgentFinished)
    ctx.agent_is_running = true;
    ctx.agent_cancel_requested.store(false, Ordering::Relaxed);
    ctx.agent_cancel_tx = Some(cancel_tx);
    ctx.agent_start_message_count = prev_len;

    // Clear paste state
    ctx.tui.paste_accumulator.clear();
    ctx.tui.last_paste_time = None;

    // Spawn watcher task: awaits agent completion and sends AgentFinished through cmd_rx.
    // This is the EventStream completion signal — the main loop handles it like
    // any other TuiCommand event, keeping the loop flat/non-nested.
    let cmd_tx_spawn = ctx.cmd_tx.clone();
    let cancel_flag = ctx.agent_cancel_requested.clone();
    tokio::spawn(async move {
        let result = match agent_handle.await {
            Ok(r) => r,
            Err(join_err) => Err(format!("Agent task failed: {}", join_err)),
        };
        let _ = cmd_tx_spawn.send(TuiCommand::AgentFinished {
            result,
            prev_len,
            cancel_requested: cancel_flag.load(Ordering::Relaxed),
        });
    });

    // Return immediately — agent events flow through cmd_rx to the main loop
}
