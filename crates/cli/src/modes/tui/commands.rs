use pick_tui::app::TuiApp;
use pick_tui::components::QuestionDialog;

use super::types::TuiCommand;

/// Apply a TuiCommand to the TUI (sync, no session persistence).
/// Used in the input loop and its drain.
pub(crate) fn apply_tui_command(tui: &mut TuiApp, cmd: TuiCommand) {
    match cmd {
        TuiCommand::StreamContent(text) => tui.stream_content(&text),
        TuiCommand::AppendContent(text) => tui.append_content(&text),
        TuiCommand::ToolExecutionStart {
            tool_call_id,
            tool_name,
            args,
        } => {
            if tool_name != "question" && tool_name != "todo_plan" {
                tui.add_tool_execution(&tool_call_id, &tool_name, args);
            }
        }
        TuiCommand::ToolExecutionUpdate {
            tool_call_id,
            partial_output,
        } => {
            tui.update_tool_execution_output(&tool_call_id, &partial_output);
        }
        TuiCommand::ToolExecutionEnd {
            tool_call_id,
            tool_name,
            output,
            is_error,
        } => {
            if tool_name != "question" && tool_name != "todo_plan" {
                tui.update_tool_execution(&tool_call_id, &output, is_error);
            }
        }
        TuiCommand::EndTurn => tui.finalize_turn(),
        TuiCommand::UpdateTodos(todos) => {
            tui.set_todo_items(todos);
        }
        TuiCommand::SetStatus(msg) => {
            tui.set_status(Some(&msg));
        }
        TuiCommand::ClearStatus => {
            tui.set_status(None);
        }
        TuiCommand::SetSessionTitle(title) => {
            tui.set_session_name(title);
        }
        TuiCommand::ShowQuestions {
            prompts,
            response_tx,
        } => {
            let qdata: Vec<(String, String, Vec<(String, String)>, bool)> = prompts
                .into_iter()
                .map(|q| {
                    let opts: Vec<(String, String)> = q
                        .options
                        .into_iter()
                        .map(|o| (o.label, o.description))
                        .collect();
                    (q.question, q.header, opts, q.multiple)
                })
                .collect();
            let dialog = QuestionDialog::new(qdata);
            tui.question_dialog = Some(dialog);
            tui.question_response_tx = Some(response_tx);
            tui.state = pick_tui::app::AppState::Questioning;
        }
        TuiCommand::RequestApproval {
            tool_name,
            tool_args,
            permission,
            response_tx,
        } => {
            let qdata = vec![(
                format!("Tool '{}' requires permission ({})", tool_name, permission),
                "Permission Request".to_string(),
                vec![
                    (
                        "Allow".to_string(),
                        format!("Allow {} with args: {}", tool_name, tool_args),
                    ),
                    ("Deny".to_string(), "Reject this tool call".to_string()),
                ],
                false,
            )];
            let dialog = QuestionDialog::new(qdata);
            tui.question_dialog = Some(dialog);
            tui.question_response_tx = Some(response_tx);
            tui.state = pick_tui::app::AppState::Questioning;
        }
        TuiCommand::GoalUpdated(goal) => {
            apply_goal_update(tui, &goal);
        }
        TuiCommand::QueueUpdate { .. } => {
            // Queue info is rendered via pending_user_messages above the editor,
            // no need to set status text that would collide with timer display.
        }
        // AgentFinished is handled directly in runner.rs, not here
        TuiCommand::AgentFinished { .. } => {}
        TuiCommand::SteeringMessageConsumed(text) => {
            // A queued message was consumed by the agent — move from pending to chat
            if let Some(pos) = tui.pending_user_messages.iter().position(|m| m == &text) {
                tui.pending_user_messages.remove(pos);
            }
            tui.chat.add_user_message(&text);
        }
    }
}

/// Apply a TuiCommand to the TUI, with session name persistence.
/// Used in the agent execution loop and its post-agent drain.
pub(crate) async fn apply_tui_command_persist(
    tui: &mut TuiApp,
    session_mgr: &mut pick_agent::session::SessionManager,
    cmd: TuiCommand,
) {
    match cmd {
        TuiCommand::SetSessionTitle(title) => {
            tui.set_session_name(title.clone());
            if session_mgr.get_session_name() != Some(title.as_str())
                && let Err(e) = session_mgr.append_session_info(&title).await
            {
                tui.show_error(&format!("Failed to persist session title: {}", e));
            }
        }
        other => apply_tui_command(tui, other),
    }
}

/// Apply goal update display to TUI
pub(crate) fn apply_goal_update(tui: &mut TuiApp, goal: &serde_json::Value) {
    if let Some(objective) = goal.get("objective").and_then(|v| v.as_str()) {
        let short = if objective.len() > 40 {
            format!("{}...", objective.chars().take(13).collect::<String>())
        } else {
            objective.to_string()
        };
        let status = goal.get("status").and_then(|v| v.as_str()).unwrap_or("");
        let icon = match status {
            "active" => "🎯",
            "paused" => "⏸",
            "budget_limited" => "💰",
            "complete" => "✅",
            _ => "🎯",
        };
        tui.set_goal_status(Some(&format!("{} {}", icon, short)));
    }
}

/// Show a question dialog in the TUI (converts QuestionPrompt vec to dialog)
pub(crate) fn show_question_dialog(
    tui: &mut TuiApp,
    prompts: Vec<pick_agent::core::state::QuestionPrompt>,
    response_tx: tokio::sync::oneshot::Sender<Result<Vec<Vec<String>>, String>>,
) {
    let qdata: Vec<(String, String, Vec<(String, String)>, bool)> = prompts
        .into_iter()
        .map(|q| {
            let opts: Vec<(String, String)> = q
                .options
                .into_iter()
                .map(|o| (o.label, o.description))
                .collect();
            (q.question, q.header, opts, q.multiple)
        })
        .collect();
    let dialog = QuestionDialog::new(qdata);
    tui.question_dialog = Some(dialog);
    tui.question_response_tx = Some(response_tx);
    tui.state = pick_tui::app::AppState::Questioning;
}

/// Drain all pending TuiCommands from cmd_rx (sync variant, no persist).
/// Returns true if the channel is closed (should quit).
pub(crate) fn drain_commands(
    tui: &mut TuiApp,
    cmd_rx: &mut tokio::sync::mpsc::UnboundedReceiver<TuiCommand>,
) -> bool {
    loop {
        match cmd_rx.try_recv() {
            Ok(cmd) => apply_tui_command(tui, cmd),
            Err(_) => return false,
        }
    }
}

/// Drain all pending TuiCommands from cmd_rx (async variant, with persist).
pub(crate) async fn drain_commands_persist(
    tui: &mut TuiApp,
    session_mgr: &mut pick_agent::session::SessionManager,
    cmd_rx: &mut tokio::sync::mpsc::UnboundedReceiver<TuiCommand>,
) -> bool {
    loop {
        match cmd_rx.try_recv() {
            Ok(cmd) => apply_tui_command_persist(tui, session_mgr, cmd).await,
            Err(_) => return false,
        }
    }
}
