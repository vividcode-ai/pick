use pick_agent::session::entries::SessionEntryKind;
use pick_agent::session::{SessionEntry, SessionManager};
use pick_ai::types::{ContentBlock, Message};

use super::context::TuiContext;
use super::message_utils;

/// Build SessionData from TuiContext and export to HTML file.
/// Returns the HTML string on success.
fn export_session_html(ctx: &TuiContext, output_path: &std::path::Path) -> Result<String, String> {
    use crate::core::export_html::export_html::{
        SessionData, ToolDef, derive_export_colors, export_session_to_html,
    };

    let header = ctx
        .session_manager
        .header()
        .map(|h| {
            serde_json::json!({
                "id": h.id,
                "version": h.version,
                "timestamp": h.created_at,
                "createdAt": h.created_at,
                "updatedAt": h.updated_at,
                "cwd": h.cwd,
                "model": h.model,
                "provider": h.provider,
            })
        })
        .unwrap_or(serde_json::Value::Null);

    let leaf_id = ctx.session_manager.get_leaf_id().map(|s| s.to_string());
    let entries: Vec<serde_json::Value> = ctx
        .session_manager
        .entries()
        .iter()
        .map(|entry| {
            match &entry.kind {
                SessionEntryKind::Message(msg) => {
                    let role = match msg.role.as_str() {
                        "tool_result" => "toolResult",
                        r => r,
                    };
                    let mut message_obj = serde_json::json!({
                        "role": role,
                        "content": msg.content,
                        "timestamp": entry.timestamp,
                    });
                    // Lowercase stop_reason to match pi format (e.g. "stop" not "Stop")
                    if let Some(sr) = &msg.stop_reason {
                        message_obj["stopReason"] = serde_json::Value::String(sr.to_lowercase());
                    }
                    if let Some(api) = &msg.api {
                        message_obj["api"] = serde_json::Value::String(api.clone());
                    }
                    if let Some(model) = &msg.model {
                        message_obj["model"] = serde_json::Value::String(model.clone());
                    }
                    if let Some(provider) = &msg.provider {
                        message_obj["provider"] = serde_json::Value::String(provider.clone());
                    }
                    // Remap usage fields from snake_case to camelCase (JS expects)
                    if let Some(usage) = &msg.usage {
                        if let Some(obj) = usage.as_object() {
                            let mut u = serde_json::Map::new();
                            let fields: &[(&str, &str)] = &[
                                ("input", "input"),
                                ("output", "output"),
                                ("cache_read", "cacheRead"),
                                ("cache_write", "cacheWrite"),
                                ("total_tokens", "totalTokens"),
                            ];
                            for &(from, to) in fields {
                                if let Some(v) = obj.get(from) {
                                    u.insert(to.into(), v.clone());
                                }
                            }
                            // Remap cost sub-fields as well
                            if let Some(cost) = obj.get("cost") {
                                if let Some(cost_obj) = cost.as_object() {
                                    let mut c = serde_json::Map::new();
                                    let cost_fields: &[(&str, &str)] = &[
                                        ("input", "input"),
                                        ("output", "output"),
                                        ("cache_read", "cacheRead"),
                                        ("cache_write", "cacheWrite"),
                                        ("total", "total"),
                                    ];
                                    for &(from, to) in cost_fields {
                                        if let Some(v) = cost_obj.get(from) {
                                            c.insert(to.into(), v.clone());
                                        }
                                    }
                                    u.insert("cost".into(), serde_json::Value::Object(c));
                                } else {
                                    u.insert("cost".into(), cost.clone());
                                }
                            }
                            message_obj["usage"] = serde_json::Value::Object(u);
                        }
                    }
                    serde_json::json!({
                        "id": entry.id,
                        "parentId": entry.parent_id,
                        "timestamp": entry.timestamp,
                        "type": "message",
                        "message": message_obj,
                    })
                }
                SessionEntryKind::Compaction(c) => serde_json::json!({
                    "id": entry.id,
                    "parentId": entry.parent_id,
                    "timestamp": entry.timestamp,
                    "type": "compaction",
                    "tokensBefore": c.token_count,
                    "summary": c.summary,
                }),
                SessionEntryKind::BranchSummary(b) => serde_json::json!({
                    "id": entry.id,
                    "parentId": entry.parent_id,
                    "timestamp": entry.timestamp,
                    "type": "branch_summary",
                    "summary": b.summary,
                }),
                SessionEntryKind::ModelChange(mc) => serde_json::json!({
                    "id": entry.id,
                    "parentId": entry.parent_id,
                    "timestamp": entry.timestamp,
                    "type": "model_change",
                    "provider": "",
                    "modelId": mc.to,
                }),
                SessionEntryKind::ThinkingLevelChange(tc) => serde_json::json!({
                    "id": entry.id,
                    "parentId": entry.parent_id,
                    "timestamp": entry.timestamp,
                    "type": "thinking_level_change",
                    "thinkingLevel": tc.to,
                }),
                SessionEntryKind::Custom(c) => serde_json::json!({
                    "id": entry.id,
                    "parentId": entry.parent_id,
                    "timestamp": entry.timestamp,
                    "type": "custom_message",
                    "customType": c.kind,
                    "content": c.data,
                    "display": true,
                }),
                SessionEntryKind::Label(l) => serde_json::json!({
                    "id": entry.id,
                    "parentId": entry.parent_id,
                    "timestamp": entry.timestamp,
                    "type": "label",
                    "targetId": l.target_id,
                    "label": l.label,
                }),
                SessionEntryKind::Goal(g) => serde_json::json!({
                    "id": entry.id,
                    "parentId": entry.parent_id,
                    "timestamp": entry.timestamp,
                    "type": "goal",
                    "status": g.status,
                    "objective": g.objective,
                }),
                SessionEntryKind::TodoUpdate(_t) => serde_json::json!({
                    "id": entry.id,
                    "parentId": entry.parent_id,
                    "timestamp": entry.timestamp,
                    "type": "todo_update",
                }),
                SessionEntryKind::SessionInfo(s) => serde_json::json!({
                    "id": entry.id,
                    "parentId": entry.parent_id,
                    "timestamp": entry.timestamp,
                    "type": "session_info",
                    "name": s.name,
                }),
                SessionEntryKind::LeafChange(lc) => serde_json::json!({
                    "id": entry.id,
                    "parentId": entry.parent_id,
                    "timestamp": entry.timestamp,
                    "type": "leaf_change",
                    "targetId": lc.to,
                }),
                SessionEntryKind::AgentModeChange(ac) => serde_json::json!({
                    "id": entry.id,
                    "parentId": entry.parent_id,
                    "timestamp": entry.timestamp,
                    "type": "agent_mode_change",
                    "from": ac.from,
                    "to": ac.to,
                }),
            }
        })
        .collect();

    let tools: Option<Vec<ToolDef>> = if ctx.tools.is_empty() {
        None
    } else {
        Some(
            ctx.tools
                .iter()
                .map(|t| ToolDef {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: Some(serde_json::to_value(&t.parameters).unwrap_or_default()),
                })
                .collect(),
        )
    };

    let session_data = SessionData {
        header,
        entries,
        leaf_id,
        system_prompt: Some(ctx.system_prompt.clone()),
        tools,
        rendered_tools: None,
    };

    let theme_colors = crate::core::theme::loader::get_resolved_theme_colors(None);
    let user_msg_bg = theme_colors
        .get("userMessageBg")
        .cloned()
        .unwrap_or_else(|| "#343541".to_string());
    let colors = derive_export_colors(&user_msg_bg);
    export_session_to_html(&session_data, &theme_colors, &colors, Some(output_path))
}

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
                match export_session_html(ctx, &p) {
                    Ok(_) => ctx.tui.chat.add_system_message(&format!(
                        "Session exported to: \x1b[1m{}\x1b[0m",
                        p.display()
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
    // ── Phase 1: Verify GitHub CLI availability and auth ──────────────
    let gh_check = tokio::process::Command::new("gh")
        .args(["auth", "status"])
        .output()
        .await;

    match gh_check {
        Ok(output) if output.status.success() => {
            // gh is installed and authenticated — proceed
        }
        Ok(_) => {
            ctx.tui.chat.add_system_message(
                "GitHub CLI is not authenticated. Run \x1b[1mgh auth login\x1b[0m first.",
            );
            return;
        }
        Err(_) => {
            ctx.tui.chat.add_system_message(
                "GitHub CLI (\x1b[1mgh\x1b[0m) is not installed. Install it from \x1b[1mhttps://cli.github.com/\x1b[0m",
            );
            return;
        }
    }

    // ── Phase 2: Export session to HTML ───────────────────────────────
    let session_id = ctx
        .session_manager
        .header()
        .map(|h| h.id.as_str())
        .unwrap_or("unknown")
        .to_string();
    let tmp_path = std::env::temp_dir().join(format!("Pick-session-{}.html", session_id));

    ctx.tui
        .chat
        .add_system_message("Exporting session to HTML...");

    match export_session_html(ctx, &tmp_path) {
        Ok(_html) => {
            // ── Phase 3: Create secret GitHub gist ───────────────────
            ctx.tui
                .chat
                .add_system_message("Creating secret GitHub gist...");

            match tokio::process::Command::new("gh")
                .args([
                    "gist",
                    "create",
                    "--public=false",
                    "--desc",
                    &format!("Pick session {}", session_id),
                    tmp_path.to_str().unwrap_or(""),
                ])
                .output()
                .await
            {
                Ok(gist_output) if gist_output.status.success() => {
                    let url = String::from_utf8_lossy(&gist_output.stdout)
                        .trim()
                        .to_string();
                    ctx.tui.chat.add_system_message(&format!(
                        "Session shared as secret gist: \x1b[1m{}\x1b[0m",
                        url
                    ));
                }
                Ok(gist_output) => {
                    let stderr = String::from_utf8_lossy(&gist_output.stderr);
                    ctx.tui
                        .show_error(&format!("Failed to create gist: {}", stderr.trim()));
                }
                Err(e) => {
                    ctx.tui
                        .show_error(&format!("Failed to run gh gist create: {}", e));
                }
            }
        }
        Err(e) => {
            ctx.tui
                .show_error(&format!("Failed to export session: {}", e));
        }
    }

    // ── Phase 4: Cleanup ─────────────────────────────────────────────
    let _ = std::fs::remove_file(&tmp_path);
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
                        crate::utils::truncate_utf8(text, 80)
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
