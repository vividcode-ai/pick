use super::context::TuiContext;

/// Handle /goal slash command
pub(crate) async fn handle_goal(ctx: &mut TuiContext, args: &[String]) -> bool {
    let args_str = args.join(" ").trim().to_string();
    let goal_manager = ctx.session_manager.goal_manager();

    if args_str.is_empty() {
        match goal_manager.get() {
            Some(goal) => {
                let remaining = goal_manager
                    .remaining_tokens()
                    .map(|r| format!("\n  Remaining tokens: {}", r))
                    .unwrap_or_default();
                let label = match goal.status.as_str() {
                    "active" => "\x1b[32mactive\x1b[0m",
                    "paused" => "\x1b[33mpaused\x1b[0m",
                    "budget_limited" => "\x1b[31mbudget limited\x1b[0m",
                    "complete" => "\x1b[32mcomplete\x1b[0m",
                    "blocked" => "\x1b[31mblocked\x1b[0m",
                    s => s,
                };
                let elapsed = {
                    let secs = goal.time_used_seconds;
                    if secs < 60 {
                        format!("{}s", secs)
                    } else if secs < 3600 {
                        format!("{}m", secs / 60)
                    } else {
                        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
                    }
                };
                ctx.tui.chat.add_system_message(&format!(
                    "\x1b[1mGoal\x1b[0m  \x1b[36m{}\x1b[0m",
                    goal.objective
                ));
                ctx.tui.chat.add_system_message(&format!(
                    "  Status: {}  Tokens: {}  Time: {}{}",
                    label, goal.tokens_used, elapsed, remaining
                ));
                ctx.tui
                    .chat
                    .add_system_message("/goal edit  /goal pause  /goal resume  /goal clear");
            }
            None => {
                ctx.tui.chat.add_system_message(
                    "Usage: \x1b[33m/goal <objective>\x1b[0m \
                     to set a persistent goal for this session. \
                     Subcommands: edit, pause, resume, clear",
                );
            }
        }
        return true;
    }

    match args_str.to_ascii_lowercase().as_str() {
        "clear" => {
            ctx.session_manager.clear_goal().await.ok();
            ctx.tui
                .chat
                .add_system_message("\x1b[33mGoal cleared.\x1b[0m");
            let _ = ctx.cmd_tx.send(super::types::TuiCommand::ClearStatus);
        }
        "pause" => match goal_manager.set_paused() {
            Ok(goal) => {
                ctx.session_manager.persist_goal().await.ok();
                ctx.tui.chat.add_system_message(&format!(
                    "\x1b[33mGoal paused.\x1b[0m  {}",
                    goal.objective
                ));
            }
            Err(e) => ctx.tui.show_error(&e),
        },
        "resume" => match goal_manager.set_active() {
            Ok(goal) => {
                ctx.session_manager.persist_goal().await.ok();
                ctx.tui.chat.add_system_message(&format!(
                    "\x1b[32mGoal resumed.\x1b[0m  {}",
                    goal.objective
                ));
            }
            Err(e) => ctx.tui.show_error(&e),
        },
        "edit" => {
            if let Some(goal) = goal_manager.get() {
                ctx.tui.editor.set_text(&goal.objective);
                ctx.tui.state = pick_tui::app::AppState::Input;
            } else {
                ctx.tui.chat.add_system_message("No goal to edit.");
            }
        }
        _ => {
            if goal_manager.get().is_some() {
                ctx.tui.chat.add_system_message(
                    "A goal already exists. Use \x1b[33m/goal edit\x1b[0m \
                     to modify it or \x1b[33m/goal clear\x1b[0m first.",
                );
            } else {
                match goal_manager.create(args_str.clone(), None) {
                    Ok(_) => {
                        ctx.session_manager.persist_goal().await.ok();
                        ctx.tui
                            .chat
                            .add_system_message(&format!("\x1b[32mGoal set:\x1b[0m  {}", args_str));
                        let short = if pick_tui::utils::visible_width(&args_str) > 40 {
                            let truncated: String = args_str.chars().take(13).collect();
                            format!("{}...", truncated)
                        } else {
                            args_str.clone()
                        };
                        ctx.tui.set_goal_status(Some(&format!("🎯 {}", short)));
                        return false; // Don't add user message
                    }
                    Err(e) => ctx.tui.show_error(&e),
                }
            }
        }
    }
    true
}
