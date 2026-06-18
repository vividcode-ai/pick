use std::sync::atomic::Ordering;
use std::time::Instant;

use pick_agent::core::agent_loop::AgentRunResult;
use pick_agent::session::SessionEntry;
use pick_ai::types::{Message, UserMessage};
use pick_tui::app::TuiAction;
use tokio::sync::mpsc;

use crate::core::updates;

use super::actions_login;
use super::actions_model;
use super::actions_session;
use super::actions_settings;
use super::agent_exec;
use super::cmd_dispatch;
use super::commands;
use super::context::TuiContext;
use super::init;
use super::key_events;
use super::tree_summarize;
use super::types::*;

/// Dispatch a TuiAction to the appropriate handler
pub(crate) async fn dispatch_action(
    ctx: &mut TuiContext,
    cmd_rx: &mut mpsc::UnboundedReceiver<TuiCommand>,
    evt_rx: &mut mpsc::UnboundedReceiver<crossterm::event::Event>,
    action: TuiAction,
) -> bool {
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
        TuiAction::SelectionResult(_idx, val) => {
            handle_selection_result(ctx, &val).await;
        }
        TuiAction::Submit(text) => {
            handle_submit(ctx, cmd_rx, evt_rx, text).await;
        }
        TuiAction::UpdateResponse(update_now) => {
            if update_now {
                if let Some(action) = crate::core::update_action::get_update_action() {
                    ctx.pending_update = Some(action);
                }
            } else {
                // Dismiss this version
                updates::dismiss_version();
            }
            return true;
        }
    }
    false
}

/// Handle Interrupt action
async fn handle_interrupt(ctx: &mut TuiContext) {
    ctx.tui.chat.add_system_message("Interrupted.");
    ctx.tui.finalize_turn();
}

/// Handle SelectionResult by dispatching to the correct handler
async fn handle_selection_result(ctx: &mut TuiContext, val: &str) {
    match ctx.pending_command.as_deref() {
        Some("login") => actions_login::handle_login_selection(ctx, val).await,
        Some("login-oauth") => actions_login::handle_oauth_login(ctx, val).await,
        Some("login-apikey") => actions_login::handle_apikey_login(ctx, val),
        Some("logout") => actions_login::handle_logout(ctx, val),
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
        _ => {
            ctx.tui
                .chat
                .add_system_message(&format!("\x1b[36m→\x1b[0m \x1b[1m{}\x1b[0m", val));
        }
    }
    ctx.pending_command = None;
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
        val.splitn(2, '/').nth(1).unwrap_or(val).to_string()
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
        let select = SelectList::new(&format!("Scoped Models ({})", status), items);
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

/// Handle Submit action - the main message submission flow
async fn handle_submit(
    ctx: &mut TuiContext,
    cmd_rx: &mut mpsc::UnboundedReceiver<TuiCommand>,
    evt_rx: &mut mpsc::UnboundedReceiver<crossterm::event::Event>,
    user_text: String,
) {
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
    }

    // ---- Run the agent ----
    ctx.tui.start_agent_timer();
    let prev_len = ctx.all_messages.len();

    if !skip_user_message {
        let from_flush = ctx.tui.pending_from_flush;
        ctx.all_messages
            .push(Message::User(UserMessage::text(&user_text)));
        if from_flush {
            ctx.tui.pending_from_flush = false;
            ctx.tui.pending_user_messages.clear();
            ctx.tui.pending_submitted_count = 0;
        } else if !is_skill_expanded {
            ctx.tui.chat.add_user_message(&user_text);
        }
    }

    // Auto-title generation for first user message
    let needs_title = {
        let is_custom = ctx
            .tui
            .session_name
            .as_deref()
            .map_or(false, |n| !super::types::is_default_session_title(n));
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
            .get_api_key(&ctx.model.provider.as_str(), true)
            .await;
        agent_exec::spawn_title_generation(
            title_text,
            ctx.cmd_tx.clone(),
            ctx.model.clone(),
            api_key,
            include_str!("../../title_prompt.txt").to_string(),
        );
    }

    let _ = ctx.tui.render_with_terminal(&mut ctx.terminal_manager);
    ctx.tui.set_status(Some("Working..."));
    let _ = ctx.tui.render_with_terminal(&mut ctx.terminal_manager);

    // Refresh tools and build agent config
    ctx.tools = init::refilter_tools(&ctx.all_tools, &ctx.agent_mode, &ctx.session_manager);
    let config = agent_exec::build_agent_config(ctx, ctx.cmd_tx.clone());

    // Register TUI approval hook
    let approval_hook = super::types::TuiApprovalHook {
        cmd_tx: ctx.cmd_tx.clone(),
    };
    ctx.permission_manager
        .register_permission_hook(std::sync::Arc::new(approval_hook));

    let msgs = ctx.all_messages.clone();
    let mut agent_handle = tokio::spawn(async move {
        crate::core::agent_session::run_agent_loop_with_retry_and_continuation(
            config,
            msgs,
            Default::default(),
            None,
        )
        .await
    });

    // Clear paste state before agent loop to avoid stale accumulator content
    ctx.tui.paste_accumulator.clear();
    ctx.tui.last_paste_time = None;

    // Process events while agent runs
    let mut should_quit = false;
    let mut agent_result: Option<Result<AgentRunResult, String>> = None;
    let mut tick_interval = tokio::time::interval(std::time::Duration::from_millis(100));
    let mut needs_render = true;

    loop {
        if needs_render {
            if ctx
                .tui
                .render_with_terminal(&mut ctx.terminal_manager)
                .is_err()
            {
                should_quit = true;
                agent_handle.abort();
                break;
            }
            needs_render = false;
        }

        tokio::select! {
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(cmd) => {
                        commands::apply_tui_command(&mut ctx.tui, cmd);
                        commands::drain_commands(&mut ctx.tui, cmd_rx);
                    }
                    None => {
                        should_quit = true;
                        agent_handle.abort();
                        break;
                    }
                }
                needs_render = true;
            }

            _ = tokio::signal::ctrl_c() => {
                should_quit = true;
                agent_handle.abort();
                break;
            }

            evt = evt_rx.recv() => {
                match evt {
                    Some(crossterm::event::Event::Key(key)) => {
                        let now = Instant::now();
                        match key_events::process_key_event_during_agent(&mut ctx.tui, key, now) {
                            Some(TuiAction::Quit) => {
                                should_quit = true;
                                agent_handle.abort();
                                break;
                            }
                            _ => {}
                        }
                        // Drain remaining events
                        let mut abort = false;
                        key_events::drain_key_events_during_agent(&mut ctx.tui, evt_rx, now, &mut abort);
                        if abort {
                            agent_handle.abort();
                            break;
                        }
                        needs_render = true;
                    }
                    Some(crossterm::event::Event::Resize(_, _)) => { needs_render = true; }
                    Some(crossterm::event::Event::Paste(text)) => {
                        ctx.tui.handle_paste(&text);
                        needs_render = true;
                    }
                    Some(_) => {}
                    None => {
                        should_quit = true;
                        agent_handle.abort();
                        break;
                    }
                }
            }

            _ = tick_interval.tick() => {
                ctx.tui.advance_spinner();
                ctx.tui.update_terminal_title();
                needs_render = true;
            }

            result = &mut agent_handle => {
                agent_result = Some(match result {
                    Ok(r) => r,
                    Err(join_err) => Err(format!("Agent task failed: {}", join_err)),
                });
                break;
            }
        }
    }

    if should_quit {
        return; // Will break outer loop
    }

    // Post-agent drain
    commands::drain_commands_persist(&mut ctx.tui, &mut ctx.session_manager, cmd_rx).await;
    ctx.tui.chat.discard_active_stream();

    // Process agent result
    match agent_result {
        Some(Ok(result)) => {
            for msg in &result.messages[prev_len..] {
                if let Err(e) = ctx.session_manager.append(SessionEntry::from(msg)).await {
                    ctx.tui
                        .show_error(&format!("Session persist failed: {}", e));
                }
            }
            ctx.all_messages = result.messages;
            ctx.tui.show_usage(result.usage.input, result.usage.output);
            let pct =
                (result.usage.total_tokens as f64) / (ctx.model.context_window as f64) * 100.0;
            ctx.tui
                .set_context_info(Some(pct), ctx.model.context_window);
            agent_exec::auto_compact_session(ctx).await;
        }
        Some(Err(e)) => {
            ctx.tui.show_error(&e);
        }
        None => {
            ctx.was_interrupted.store(true, Ordering::Relaxed);
            ctx.tui.chat.add_system_message("Interrupted.");
        }
    }

    ctx.tui.stop_agent_timer();
    ctx.tui.set_status(None);
    ctx.tui.finalize_turn();
}
