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
use super::context::TuiContext;
use super::init;
use super::key_events;
use super::types::TuiCommand;

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
    sandbox_enabled: Arc<AtomicBool>,
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
        sandbox_enabled,
    )
    .await;

    // Git branch refresh timer (1-second interval)
    let mut git_timer = tokio::time::interval(Duration::from_secs(1));
    git_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // Spinner animation timer (100ms interval)
    let mut spinner_timer = tokio::time::interval(Duration::from_millis(100));
    spinner_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

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

            // Check for pending command continuation
            if let Some(text) = ctx.pending_command.take() {
                if text.starts_with('/') || ctx.tui.state != pick_tui::app::AppState::Input {
                    ctx.pending_command = Some(text);
                }
            }

            tokio::select! {
                biased;

                _ = git_timer.tick() => {
                    let branch = detect_git_branch(&ctx.cwd);
                    ctx.tui.set_git_branch(branch);
                }

                _ = spinner_timer.tick() => {
                    ctx.tui.advance_spinner();
                    ctx.tui.update_terminal_title();
                }

                // OS-level Ctrl+C signal fallback. On Windows, crossterm raw
                // mode doesn't always prevent Ctrl+C from generating a real
                // CTRL_C_EVENT that bypasses the keyboard reader thread and
                // reaches tokio's signal handler. This branch catches that
                // signal and triggers a clean exit through the normal path.
                _ = ctx.ctrl_c_rx.changed() => {
                    if *ctx.ctrl_c_rx.borrow() {
                        break 'input TuiAction::Quit;
                    }
                }

                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(TuiCommand::SetSessionTitle(title)) => {
                            commands::apply_tui_command(&mut ctx.tui, TuiCommand::SetSessionTitle(title.clone()));
                            // Persist immediately so the session picker sees the real title
                            if let Err(e) = ctx.session_manager.append_session_info(&title).await {
                                ctx.tui.show_error(&format!("Failed to persist session title: {}", e));
                            }
                        }
                        Some(TuiCommand::AgentFinished { result, prev_len, cancel_requested }) => {
                            action_dispatch::handle_agent_finished(&mut ctx, result, prev_len, cancel_requested).await;
                        }
                        Some(TuiCommand::ShareResult { url, error }) => {
                            handle_share_result(&mut ctx, url, error);
                            // Force an immediate render so the restored editor
                            // is visible without waiting for the next loop iteration.
                            let _ = ctx.tui.render_with_terminal(&mut ctx.terminal_manager);
                        }
                        Some(cmd) => commands::apply_tui_command(&mut ctx.tui, cmd),
                        None => break 'input TuiAction::Quit,
                    }
                }

                evt = evt_rx.recv() => {
                    match evt {
                        Some(crossterm::event::Event::Key(key))
                            if key.kind == crossterm::event::KeyEventKind::Press
                                && key.code == crossterm::event::KeyCode::Enter =>
                        {
                            let now = Instant::now();
                            // Peek for pending \n/\r (dual-event Windows terminal
                            // bug: some terminals generate both Enter + \n for a
                            // single modified-Enter keystroke with arbitrary ordering).
                            // If \n is queued behind Enter, consume it and let the
                            // Enter go through normal process_key_event which handles
                            // Shift+Enter (newline) vs Ctrl+Enter (submit) correctly.
                            match evt_rx.try_recv() {
                                Ok(crossterm::event::Event::Key(nl_key))
                                    if nl_key.kind == crossterm::event::KeyEventKind::Press
                                        && matches!(nl_key.code, crossterm::event::KeyCode::Char('\n') | crossterm::event::KeyCode::Char('\r')) =>
                                {
                                    // Dual-event: capture modifiers from the \n
                                    // BEFORE consuming it.  On Windows without
                                    // keyboard enhancement, the \n carries the
                                    // Shift/Ctrl state via GetAsyncKeyState.
                                    // Without this, the subsequent Enter+NONE
                                    // processing loses the modifier and submits.
                                    key_events::track_modifiers(&mut ctx.tui, &nl_key, now);
                                    // \n consumed, Enter goes through normal
                                    // processing which uses the tracked modifiers.
                                    if let Some(action) = key_events::process_key_event(&mut ctx.tui, key, now) {
                                        break 'input action;
                                    }
                                }
                                Ok(crossterm::event::Event::Key(other_key))
                                    if other_key.kind == crossterm::event::KeyEventKind::Press =>
                                {
                                    // Peeked a non-\n key event. Process both
                                    // the original Enter and the consumed event.
                                    if let Some(action) = key_events::process_key_event(&mut ctx.tui, key, now) {
                                        break 'input action;
                                    }
                                    if let Some(action) = key_events::process_key_event(&mut ctx.tui, other_key, now) {
                                        break 'input action;
                                    }
                                }
                                Ok(_) => {
                                    // Non-key event consumed (Resize, Paste).
                                    // Process original Enter normally.
                                    if let Some(action) = key_events::process_key_event(&mut ctx.tui, key, now) {
                                        break 'input action;
                                    }
                                }
                                Err(_) => {
                                    // No pending events — process Enter normally.
                                    if let Some(action) = key_events::process_key_event(&mut ctx.tui, key, now) {
                                        break 'input action;
                                    }
                                }
                            }
                        }
                        Some(crossterm::event::Event::Key(key)) => {
                            let now = Instant::now();
                            if let Some(action) = key_events::process_key_event(&mut ctx.tui, key, now) {
                                break 'input action;
                            }
                        }
                        Some(crossterm::event::Event::Resize(_, _)) => {}
                        Some(crossterm::event::Event::Paste(text)) => {
                            ctx.tui.force_flush_paste_accumulator();
                            ctx.tui.handle_paste(&text);
                            ctx.tui.last_paste_time = Some(Instant::now());
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

            // Drain remaining command events — AgentFinished must be
            // handled here (not in apply_tui_command which treats it as
            // no-op) to ensure usage display and session persistence.
            // Without this, AgentFinished can be lost when it arrives in
            // the channel alongside EndTurn and gets picked up by drain
            // instead of the select! branch above.
            loop {
                match cmd_rx.try_recv() {
                    Ok(TuiCommand::AgentFinished {
                        result,
                        prev_len,
                        cancel_requested,
                    }) => {
                        action_dispatch::handle_agent_finished(
                            &mut ctx,
                            result,
                            prev_len,
                            cancel_requested,
                        )
                        .await;
                    }
                    Ok(TuiCommand::ShareResult { url, error }) => {
                        handle_share_result(&mut ctx, url, error);
                        let _ = ctx.tui.render_with_terminal(&mut ctx.terminal_manager);
                    }
                    Ok(cmd) => commands::apply_tui_command(&mut ctx.tui, cmd),
                    Err(_) => break,
                }
            }
        };

        let should_quit = action_dispatch::dispatch_action(&mut ctx, action).await;
        if should_quit {
            break;
        }
    }

    // Phase 3: Cleanup
    cleanup::cleanup_tui_mode(&mut ctx);

    // Return pending update action
    ctx.pending_update.take()
}

/// Handle ShareResult from a background share operation.
/// Restores the editor, clears share state, and shows the result in chat.
fn handle_share_result(ctx: &mut TuiContext, url: Option<String>, error: Option<String>) {
    // Restore editor if this is a normal completion (not pre-handled by Esc cancel).
    // If already cancelled (share_cancel_tx == None), editor was restored by handle_interrupt.
    if ctx.share_cancel_tx.take().is_some() {
        ctx.share_saved_editor_text.clear();
        ctx.tui.editor.clear();
        ctx.tui.state = pick_tui::app::AppState::Input;
    }

    // Clear share-in-progress flag so the spinner stops
    ctx.tui.share_in_progress = false;

    match (url, error) {
        (Some(url), _) => {
            // OSC 8 hyperlink — the only reliably clickable link in TUI
            // scrollback. Emit \x1b[24m and \x1b[4:0m inside the hyperlink
            // span to remove the text underline; the terminal's link layer
            // may still show a subtle dotted line (unavoidable for OSC 8).
            use ratatui::text::{Line, Span};
            let clickable = format!(
                "\x1b]8;;{}\x1b\\\x1b[24m\x1b[4:0m{}\x1b]8;;\x1b\\",
                &url, &url
            );
            ctx.tui.pending_history_lines.push(Line::from(vec![
                Span::raw("🔗 Session shared: "),
                Span::raw(clickable),
            ]));
        }
        (_, Some(err)) => {
            ctx.tui.show_error(&format!("Share failed: {}", err));
        }
        (None, None) => {
            // Cancelled — message already shown by handle_interrupt
        }
    }
    ctx.tui.finalize_turn();
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
