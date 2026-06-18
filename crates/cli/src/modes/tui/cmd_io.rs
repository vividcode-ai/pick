use pick_agent::session::{SessionEntry, SessionManager};
use pick_ai::types::{ContentBlock, Message};

use super::context::TuiContext;
use super::message_utils;

/// Handle /export slash command
pub(crate) async fn handle_export(ctx: &mut TuiContext, args: &[String]) {
    let path_arg = args.join(" ");
    let is_html = path_arg.is_empty() || !path_arg.ends_with(".jsonl");
    let export_path = if path_arg.is_empty() {
        let session_id = ctx
            .session_manager
            .header()
            .map(|h| h.id.as_str())
            .unwrap_or("unknown");
        let ext = if is_html { "html" } else { "jsonl" };
        std::env::current_dir()
            .ok()
            .map(|p| p.join(format!("session-{}.{}", session_id, ext)))
    } else {
        Some(std::path::PathBuf::from(&path_arg))
    };

    match export_path {
        Some(p) => {
            if is_html {
                let header = ctx
                    .session_manager
                    .header()
                    .map(|h| {
                        serde_json::json!({
                            "id": h.id,
                            "version": h.version,
                            "createdAt": h.created_at,
                            "updatedAt": h.updated_at,
                            "cwd": h.cwd,
                            "model": h.model,
                            "provider": h.provider,
                        })
                    })
                    .unwrap_or(serde_json::Value::Null);

                let entries: Vec<serde_json::Value> = ctx
                    .all_messages
                    .iter()
                    .map(|msg| {
                        let id = uuid::Uuid::now_v7().to_string();
                        let message_val = match msg {
                            Message::User(u) => {
                                serde_json::json!({"role": "user", "content": u.content})
                            }
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
                                },
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

                use crate::core::export_html::export_html::{
                    ExportColors, SessionData, export_session_to_html,
                };
                let session_data = SessionData {
                    header,
                    entries,
                    leaf_id: None,
                    system_prompt: Some(ctx.system_prompt.clone()),
                    tools: None,
                    rendered_tools: None,
                };
                let colors = ExportColors::default();
                let theme_colors = std::collections::HashMap::new();
                match export_session_to_html(&session_data, &theme_colors, &colors, Some(&p)) {
                    Ok(path) => ctx.tui.chat.add_system_message(&format!(
                        "Session exported to: \x1b[1m{}\x1b[0m",
                        path
                    )),
                    Err(e) => ctx.tui.show_error(&format!("Export failed: {}", e)),
                }
            } else {
                match message_utils::export_messages_to_jsonl(&ctx.all_messages, &p) {
                    Ok(()) => ctx.tui.chat.add_system_message(&format!(
                        "Session exported to: \x1b[1m{}\x1b[0m",
                        p.display()
                    )),
                    Err(e) => ctx.tui.show_error(&format!("Export failed: {}", e)),
                }
            }
        }
        None => ctx.tui.show_error("Could not determine export path."),
    }
}

/// Handle /import slash command
pub(crate) async fn handle_import(ctx: &mut TuiContext, args: &[String]) {
    let path = args.join(" ");
    if path.is_empty() {
        ctx.tui
            .chat
            .add_system_message("Usage: /import <path.jsonl>");
    } else if !path.ends_with(".jsonl") {
        ctx.tui
            .chat
            .add_system_message("Only \x1b[1m.jsonl\x1b[0m files are supported for import.");
    } else {
        let import_path = std::path::PathBuf::from(&path);
        let cwd = std::env::current_dir().unwrap_or_default();
        let session_dir = cwd.join(".pick").join("sessions");

        match SessionManager::open(import_path.clone(), cwd.clone()).await {
            Ok(new_mgr) => {
                let entries = new_mgr.entries().to_vec();
                let msgs = message_utils::entries_to_messages(&entries);
                if !msgs.is_empty() {
                    ctx.session_manager = new_mgr;
                    ctx.all_messages = msgs;
                    ctx.tui.chat.add_system_message(&format!(
                        "Imported \x1b[1m{}\x1b[0m messages from session.",
                        ctx.all_messages.len()
                    ));
                } else {
                    ctx.tui
                        .chat
                        .add_system_message("Session file contains no message entries.");
                }
            }
            Err(_) => match std::fs::read_to_string(&import_path) {
                Ok(content) => {
                    let msgs: Vec<Message> = content
                        .lines()
                        .filter(|l| !l.trim().is_empty())
                        .filter_map(|l| serde_json::from_str(l).ok())
                        .collect();
                    let msg_count = msgs.len();
                    if msg_count == 0 {
                        ctx.tui.chat.add_system_message(&format!(
                            "No parseable messages in '\x1b[1m{}\x1b[0m'.",
                            path
                        ));
                    } else {
                        match SessionManager::create(cwd, Some(session_dir)).await {
                            Ok(mut sess) => {
                                for msg in &msgs {
                                    let _ = sess.append(SessionEntry::from(msg)).await;
                                }
                                ctx.all_messages = msgs;
                                ctx.session_manager = sess;
                                ctx.tui.chat.add_system_message(&format!(
                                    "Imported \x1b[1m{}\x1b[0m messages.",
                                    msg_count
                                ));
                            }
                            Err(e) => ctx
                                .tui
                                .show_error(&format!("Failed to create session: {}", e)),
                        }
                    }
                }
                Err(e) => ctx
                    .tui
                    .show_error(&format!("Cannot read '{}': {}", path, e)),
            },
        }
    }
}

/// Handle /share slash command
pub(crate) async fn handle_share(ctx: &mut TuiContext) {
    let gh_check = std::process::Command::new("gh")
        .args(["auth", "status"])
        .output();
    match gh_check {
        Ok(output) if output.status.success() => {
            let session_id = ctx
                .session_manager
                .header()
                .map(|h| h.id.as_str())
                .unwrap_or("unknown");
            let tmp_path = std::env::temp_dir().join(format!("Pick-session-{}.md", session_id));
            let md_content: String = ctx
                .all_messages
                .iter()
                .filter_map(|msg| match msg {
                    Message::User(u) => {
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
                            .collect::<Vec<_>>()
                            .join("\n");
                        Some(format!("## User\n\n{}\n", text))
                    }
                    Message::Assistant(a) => {
                        let text: String = a
                            .content
                            .iter()
                            .filter_map(|b| {
                                if let ContentBlock::Text(t) = b {
                                    Some(t.text.clone())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        if text.is_empty() {
                            None
                        } else {
                            Some(format!("## Assistant\n\n{}\n", text))
                        }
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n---\n");
            match std::fs::write(&tmp_path, &md_content) {
                Ok(()) => {
                    match std::process::Command::new("gh")
                        .args([
                            "gist",
                            "create",
                            "--public=false",
                            "--desc",
                            &format!("Pick session {}", session_id),
                            tmp_path.to_str().unwrap_or(""),
                        ])
                        .output()
                    {
                        Ok(gist_output) if gist_output.status.success() => {
                            let url = String::from_utf8_lossy(&gist_output.stdout)
                                .trim()
                                .to_string();
                            ctx.tui.chat.add_system_message(&format!(
                                "Session shared as gist: \x1b[1m{}\x1b[0m",
                                url
                            ));
                        }
                        Ok(gist_output) => {
                            let stderr = String::from_utf8_lossy(&gist_output.stderr);
                            ctx.tui
                                .show_error(&format!("Failed to create gist: {}", stderr.trim()));
                        }
                        Err(e) => ctx
                            .tui
                            .show_error(&format!("Failed to run gh gist create: {}", e)),
                    }
                }
                Err(e) => ctx
                    .tui
                    .show_error(&format!("Failed to write session file: {}", e)),
            }
        }
        _ => {
            ctx.tui.chat.add_system_message(
                "GitHub CLI (\x1b[1mgh\x1b[0m) is required to share sessions. Run \x1b[1mgh auth login\x1b[0m first.",
            );
        }
    }
}

/// Handle /copy slash command
pub(crate) fn handle_copy(ctx: &mut TuiContext) {
    let assistant_text: Vec<String> = ctx
        .all_messages
        .iter()
        .rev()
        .filter_map(|msg| {
            if let Message::Assistant(a) = msg {
                let texts: Vec<String> = a
                    .content
                    .iter()
                    .filter_map(|block| {
                        if let ContentBlock::Text(t) = block {
                            Some(t.text.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                if !texts.is_empty() {
                    Some(texts.join(""))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    if let Some(text) = assistant_text.first() {
        match arboard::Clipboard::new() {
            Ok(mut clipboard) => match clipboard.set_text(text.clone()) {
                Ok(()) => {
                    let preview = if text.len() > 80 {
                        format!("{}...", &text[..80])
                    } else {
                        text.clone()
                    };
                    ctx.tui.chat.add_system_message(&format!(
                        "Copied \x1b[1m{}\x1b[0m characters to clipboard.",
                        text.len()
                    ));
                    ctx.tui.chat.add_system_message(&format!(
                        "\x1b[2m  {}\x1b[0m",
                        preview.replace('\n', " ")
                    ));
                }
                Err(e) => ctx
                    .tui
                    .show_error(&format!("Clipboard write failed: {}", e)),
            },
            Err(e) => ctx
                .tui
                .show_error(&format!("Clipboard access failed: {}", e)),
        }
    } else {
        ctx.tui
            .chat
            .add_system_message("No assistant messages to copy.");
    }
}
