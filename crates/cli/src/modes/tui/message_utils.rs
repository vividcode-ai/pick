use std::path::Path;

use pick_agent::session::SessionEntryKind;
use pick_agent::session::{SessionEntry, SessionManager};
use pick_ai::types::{ContentBlock, Message, StopReason};
use pick_tui::app::TuiApp;

/// Convert SessionEntry list to Message list
pub(crate) fn entries_to_messages(entries: &[SessionEntry]) -> Vec<Message> {
    entries
        .iter()
        .filter_map(|entry| match &entry.kind {
            SessionEntryKind::Message(msg) => {
                let content: Vec<ContentBlock> =
                    serde_json::from_value(msg.content.clone()).ok()?;
                match msg.role.as_str() {
                    "user" => Some(Message::User(pick_ai::types::UserMessage {
                        role: pick_ai::types::UserMessageRole::User,
                        content,
                        timestamp: entry.timestamp,
                    })),
                    "assistant" => {
                        let usage: pick_ai::types::Usage = msg
                            .usage
                            .as_ref()
                            .and_then(|u| serde_json::from_value(u.clone()).ok())
                            .unwrap_or_else(pick_ai::types::Usage::zero);
                        let stop_reason = match msg.stop_reason.as_deref() {
                            Some("Stop") => StopReason::Stop,
                            Some("Length") => StopReason::Length,
                            Some("ToolUse") => StopReason::ToolUse,
                            Some("Error") => StopReason::Error,
                            Some("Aborted") => StopReason::Aborted,
                            _ => StopReason::Stop,
                        };
                        Some(Message::Assistant(pick_ai::types::AssistantMessage {
                            role: pick_ai::types::AssistantMessageRole::Assistant,
                            content,
                            api: msg.api.clone().unwrap_or_default(),
                            provider: msg.provider.clone().unwrap_or_default(),
                            model: msg.model.clone().unwrap_or_default(),
                            response_model: None,
                            response_id: None,
                            usage,
                            stop_reason,
                            error_message: None,
                            diagnostics: None,
                            timestamp: entry.timestamp,
                        }))
                    }
                    "tool_result" => Some(Message::ToolResult(pick_ai::types::ToolResultMessage {
                        role: pick_ai::types::ToolResultMessageRole::ToolResult,
                        tool_call_id: String::new(),
                        tool_name: String::new(),
                        content,
                        is_error: false,
                        details: None,
                        timestamp: entry.timestamp,
                    })),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect()
}

/// Extract text content from all_messages for export
pub(crate) fn export_messages_to_jsonl(messages: &[Message], path: &Path) -> Result<(), String> {
    use std::io::Write;
    let file = std::fs::File::create(path).map_err(|e| format!("Cannot create file: {}", e))?;
    let mut writer = std::io::BufWriter::new(file);
    for msg in messages {
        let line = serde_json::to_string(msg).map_err(|e| format!("Serialize error: {}", e))?;
        writeln!(writer, "{}", line).map_err(|e| format!("Write error: {}", e))?;
    }
    writer.flush().map_err(|e| format!("Flush error: {}", e))?;
    Ok(())
}

/// Load existing session history into all_messages and populate TUI chat
pub(crate) fn restore_session_history(
    tui: &mut TuiApp,
    session_manager: &SessionManager,
    initial_messages: &[Message],
    hide_thinking: bool,
    show_images: bool,
    block_images: bool,
) -> Vec<Message> {
    let session_msgs = entries_to_messages(session_manager.entries());
    for msg in &session_msgs {
        match msg {
            Message::User(u) => {
                for block in &u.content {
                    if let ContentBlock::Text(t) = block {
                        tui.chat.add_user_message(&t.text);
                    }
                }
            }
            Message::Assistant(a) => {
                let combined = super::types::combine_content_blocks(
                    &a.content,
                    hide_thinking,
                    show_images,
                    block_images,
                );
                if !combined.is_empty() {
                    tui.chat.stream_assistant_content(&combined);
                }
                tui.chat.mark_turn_end();
            }
            _ => {}
        }
    }

    for msg in initial_messages {
        if let Message::User(u) = msg {
            for block in &u.content {
                if let ContentBlock::Text(t) = block {
                    tui.chat.add_user_message(&t.text);
                }
            }
        }
    }

    let mut all_messages = session_msgs;
    all_messages.extend(initial_messages.iter().cloned());
    all_messages
}
