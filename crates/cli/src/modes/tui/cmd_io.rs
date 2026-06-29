use pick_agent::session::entries::SessionEntryKind;
use pick_agent::session::{SessionEntry, SessionManager};
use pick_ai::types::{ContentBlock, Message};

use super::context::TuiContext;
use super::message_utils;
use super::types::TuiCommand;

/// Spawn `gh gist create` in background with an HTML file path,
/// and send the result (gist URL or error) back through `cmd_tx`.
/// Checks `cancel_rx` for early cancellation (Escape key).
/// Cleans up the temp HTML file on completion or cancellation.
async fn run_share_gh_gist(
    html_path: std::path::PathBuf,
    cmd_tx: tokio::sync::mpsc::UnboundedSender<TuiCommand>,
    mut cancel_rx: tokio::sync::watch::Receiver<bool>,
) {
    let path_str = html_path.to_string_lossy().to_string();

    let mut child = match tokio::process::Command::new("gh")
        .args([
            "gist",
            "create",
            "--public=false",
            "--filename",
            "session.html",
            &path_str,
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = cmd_tx.send(TuiCommand::ShareResult {
                url: None,
                error: Some(format!("Failed to run gh: {}", e)),
            });
            return;
        }
    };

    // Read stdout/stderr before waiting (borrows child, keeps it alive for cancel)
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    if let Some(out) = child.stdout.as_mut() {
        let _ = tokio::io::AsyncReadExt::read_to_end(out, &mut stdout).await;
    }
    if let Some(err) = child.stderr.as_mut() {
        let _ = tokio::io::AsyncReadExt::read_to_end(err, &mut stderr).await;
    }

    tokio::select! {
        status = child.wait() => {
            match status {
                Ok(status) if status.success() => {
                    let url = String::from_utf8_lossy(&stdout).trim().to_string();
                    let _ = cmd_tx.send(TuiCommand::ShareResult {
                        url: Some(url),
                        error: None,
                    });
                }
                Ok(_) => {
                    let stderr_str = String::from_utf8_lossy(&stderr);
                    let stdout_str = String::from_utf8_lossy(&stdout);
                    let err_msg = if !stderr_str.trim().is_empty() {
                        stderr_str.trim().to_string()
                    } else if !stdout_str.trim().is_empty() {
                        stdout_str.trim().to_string()
                    } else {
                        "gh exited with non-zero status".to_string()
                    };
                    let _ = cmd_tx.send(TuiCommand::ShareResult {
                        url: None,
                        error: Some(err_msg),
                    });
                }
                Err(e) => {
                    let _ = cmd_tx.send(TuiCommand::ShareResult {
                        url: None,
                        error: Some(format!("gh process error: {}", e)),
                    });
                }
            }
        }
        _ = cancel_rx.changed() => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            let _ = cmd_tx.send(TuiCommand::ShareResult {
                url: None,
                error: None,
            });
        }
    }

    // Cleanup temp file
    let _ = std::fs::remove_file(&html_path);
}

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
                    if let Some(obj) = msg.usage.as_ref().and_then(|u| u.as_object()) {
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
///
/// Exports the session as a full HTML file (same format as /export),
/// then uploads it to GitHub as a secret gist for easy sharing.
///
/// Non-blocking: checks `gh` auth synchronously, then spawns the
/// gist creation in a background task. The editor shows a loading
/// spinner and the user can press Escape to cancel.
/// The result (gist URL or error) is delivered via `cmd_rx` / `ShareResult`.
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
                "GitHub CLI (\x1b[1mgh\x1b[0m) is not installed. \
                 Install it from \x1b[1mhttps://cli.github.com/\x1b[0m",
            );
            return;
        }
    }

    // ── Phase 2: Export session to HTML (reuses /export code path) ──
    let session_id = ctx
        .session_manager
        .header()
        .map(|h| h.id.as_str())
        .unwrap_or("unknown")
        .to_string();
    let tmp_path = std::env::temp_dir().join(format!("Pick-session-{}.html", session_id));

    // Remove any stale file from a previous failed share
    let _ = std::fs::remove_file(&tmp_path);

    match export_session_html(ctx, &tmp_path) {
        Ok(_) => {
            // ── Phase 3: Setup loading spinner in editor ──────────
            let cmd_tx = ctx.cmd_tx.clone();
            ctx.share_saved_editor_text = String::new();
            // Set spinner text immediately. No leading space — the braille
            // spinner character ⠋ is visually distinct from the / in /share,
            // ensuring ratatui buffer-diff properly overwrites the stale /.
            ctx.tui.editor.set_text("⠋ Creating gist…  (Esc to cancel)");
            ctx.tui.share_in_progress = true;
            ctx.tui.state = pick_tui::app::AppState::Streaming;

            // ── Phase 4: Spawn background gist creation ──────────
            let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
            ctx.share_cancel_tx = Some(cancel_tx);

            tokio::spawn(async move {
                run_share_gh_gist(tmp_path, cmd_tx, cancel_rx).await;
            });
            // ── Return immediately — result flows through cmd_rx ─
        }
        Err(e) => {
            ctx.tui
                .show_error(&format!("Failed to export session: {}", e));
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
