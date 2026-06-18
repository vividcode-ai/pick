use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::args::Args;
use crate::core::agent_mode::AgentMode;
use crate::core::auth_storage::AuthStorage;
use pick_agent::core::state::AgentTool;
use pick_agent::extensions::runner::ExtensionRunner;
use pick_agent::session::SessionManager;
use pick_ai::types::Message;
use pick_mcp::McpManager;
use pick_tui::app::TuiAction;

use super::action_dispatch;
use super::cleanup;
use super::commands;
use super::init;
use super::key_events;

use crate::core::update_action::UpdateAction;

/// Run the agent in TUI mode. Returns a pending update action if the user chose to update.
#[allow(clippy::too_many_arguments)]
pub async fn run_tui_mode(
    args: Args,
    all_tools: Arc<RwLock<Vec<AgentTool>>>,
    auth: Arc<AuthStorage>,
    session_manager: SessionManager,
    initial_messages: Vec<Message>,
    extension_runner: Option<Arc<ExtensionRunner>>,
    agent_mode: AgentMode,
    agent_registry: Arc<pick_agent::agent_registry::AgentRegistry>,
    mcp_manager: Arc<McpManager>,
    mcp_done_rx: tokio::sync::watch::Receiver<bool>,
    mcp_cancelled: Arc<AtomicBool>,
    permission_manager: Arc<pick_agent::permission::manager::PermissionManager>,
    platform_sandbox: Option<std::sync::Arc<dyn pick_agent::permission::sandbox::Sandbox>>,
) -> Option<UpdateAction> {
    // Phase 1: Initialize all TUI state
    let (mut ctx, mut cmd_rx, mut evt_rx) = init::init_tui_mode(
        args,
        all_tools,
        auth,
        session_manager,
        initial_messages,
        extension_runner,
        agent_mode,
        agent_registry,
        mcp_manager,
        mcp_done_rx,
        mcp_cancelled,
        permission_manager,
        platform_sandbox,
    )
    .await;

    // Git branch refresh timer (1-second interval)
    let mut git_timer = tokio::time::interval(Duration::from_secs(1));
    git_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Phase 2: Main interaction loop
    loop {
        // ---- Wait for user input (or process agent events) ----
        let action: TuiAction = 'input: loop {
            if ctx
                .tui
                .render_with_terminal(&mut ctx.terminal_manager)
                .is_err()
            {
                break 'input TuiAction::Quit;
            }

            // Check for pending auto-submit from pending_user_messages flush
            if let Some(text) = ctx.pending_command.take() {
                if text.starts_with('/') || ctx.tui.state != pick_tui::app::AppState::Input {
                    ctx.pending_command = Some(text);
                } else if ctx.tui.pending_from_flush {
                    break 'input TuiAction::Submit(text);
                }
            }

            tokio::select! {
                biased;

                _ = git_timer.tick() => {
                    let branch = detect_git_branch(&ctx.cwd);
                    ctx.tui.set_git_branch(branch);
                }

                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(cmd) => commands::apply_tui_command(&mut ctx.tui, cmd),
                        None => break 'input TuiAction::Quit,
                    }
                }

                evt = evt_rx.recv() => {
                    match evt {
                        Some(crossterm::event::Event::Key(key)) => {
                            let now = Instant::now();
                            if let Some(action) = key_events::process_key_event(&mut ctx.tui, key, now) {
                                break 'input action;
                            }
                        }
                        Some(crossterm::event::Event::Resize(_, _)) => {}
                        Some(crossterm::event::Event::Paste(text)) => {
                            ctx.tui.handle_paste(&text);
                        }
                        Some(_) => {}
                        None => break 'input TuiAction::Quit,
                    }
                }

                // Flush small paste accumulator content after inactivity
                _ = tokio::time::sleep(Duration::from_millis(20)) => {
                    ctx.tui.finalize_paste_accumulator(Instant::now());
                }
            }

            // Drain remaining keyboard events (paste accumulation)
            if let Some(action) =
                key_events::drain_key_events(&mut ctx.tui, &mut evt_rx, Instant::now())
            {
                break 'input action;
            }

            // Drain remaining command events
            commands::drain_commands(&mut ctx.tui, &mut cmd_rx);
        };

        let is_submit = matches!(action, TuiAction::Submit(_));
        let should_quit =
            action_dispatch::dispatch_action(&mut ctx, &mut cmd_rx, &mut evt_rx, action).await;
        if should_quit {
            break;
        }

        // Flush pending user messages after a Submit completes.
        // Then wipe all residual state to prevent terminal echo or stale
        // paste buffer content from leaking into the next input loop.
        if is_submit {
            let current_pending_count = ctx.tui.pending_user_messages.len();
            let new_count = current_pending_count.saturating_sub(ctx.tui.pending_submitted_count);
            if new_count > 0 {
                let new_msgs: Vec<String> = ctx
                    .tui
                    .pending_user_messages
                    .iter()
                    .skip(ctx.tui.pending_submitted_count)
                    .cloned()
                    .collect();
                let combined = new_msgs.join("\n");
                for msg in &new_msgs {
                    ctx.tui.editor.push_history(msg.clone());
                }
                let _ = ctx.tui.render_with_terminal(&mut ctx.terminal_manager);
                ctx.tui.paste_accumulator.clear();
                ctx.tui.last_paste_time = None;
                ctx.tui.pending_from_flush = true;
                ctx.tui.pending_submitted_count = current_pending_count;
                ctx.pending_command = Some(combined);
            } else {
                // No pending messages to flush: clear all residual state
                // to prevent terminal echo characters from the agent's
                // stdout output reaching pending_user_messages.
                ctx.tui.paste_accumulator.clear();
                ctx.tui.last_paste_time = None;
                ctx.tui.pending_user_messages.clear();
                ctx.tui.pending_submitted_count = 0;
                ctx.tui.pending_from_flush = false;
            }
        }
    }

    // Phase 3: Cleanup
    cleanup::cleanup_tui_mode(&mut ctx);

    // Return pending update action
    ctx.pending_update.take()
}

/// Read git branch from .git/HEAD for TUI footer display
fn detect_git_branch(cwd: &std::path::Path) -> Option<String> {
    let git_dir = find_git_dir(cwd)?;
    let head_path = git_dir.join("HEAD");
    let head = std::fs::read_to_string(head_path).ok()?;
    let head = head.trim();
    if let Some(ref_name) = head.strip_prefix("ref: refs/heads/") {
        Some(ref_name.to_string())
    } else if !head.is_empty() {
        Some("detached".to_string())
    } else {
        None
    }
}

/// Walk up from cwd to find .git directory
fn find_git_dir(cwd: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut dir = Some(cwd.to_path_buf());
    while let Some(d) = dir {
        let git = d.join(".git");
        if git.is_dir() {
            return Some(git);
        }
        dir = d.parent().map(|p| p.to_path_buf());
    }
    None
}
