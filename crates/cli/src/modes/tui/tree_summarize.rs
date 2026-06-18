use pick_agent::session::{BranchSummaryEntry, SessionEntry, SessionEntryKind};
use pick_ai::types::{ContentBlock, Message, UserMessage};

use super::context::TuiContext;

/// Handle tree summarization result (called after user picks "Summarize" or "No summary")
pub(crate) async fn handle_tree_summarize(ctx: &mut TuiContext, val: &str) {
    let cmd = ctx.pending_command.take();
    let target_id = cmd
        .as_deref()
        .and_then(|c| c.strip_prefix("tree-summarize:"))
        .unwrap_or("")
        .to_string();
    if target_id.is_empty() {
        return;
    }

    match val {
        "no-summary" => navigate_to(ctx, &target_id),
        "summarize" => summarize_and_navigate(ctx, &target_id).await,
        _ => navigate_to(ctx, &target_id),
    }
}

/// Generate branch summary and then navigate to target
async fn summarize_and_navigate(ctx: &mut TuiContext, target_id: &str) {
    let old_leaf = ctx.session_manager.get_leaf_id();
    let ancestor = old_leaf
        .and_then(|old| ctx.session_manager.find_common_ancestor(old, target_id));

    if let (Some(old), Some(ancestor_id)) = (old_leaf, &ancestor) {
        let entries_to_summarize = ctx
            .session_manager
            .collect_entries_for_summary(old, target_id);
        let conversation: String = entries_to_summarize
            .iter()
            .filter_map(|entry| {
                let role = match &entry.kind {
                    SessionEntryKind::Message(m) => Some(m.role.as_str()),
                    _ => None,
                };
                let text = match &entry.kind {
                    SessionEntryKind::Message(m) => match &m.content {
                        serde_json::Value::String(s) => Some(s.clone()),
                        serde_json::Value::Array(arr) => {
                            let texts: Vec<String> = arr
                                .iter()
                                .filter_map(|c| {
                                    c.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                                })
                                .collect();
                            if texts.is_empty() { None } else { Some(texts.join("\n")) }
                        }
                        _ => None,
                    },
                    SessionEntryKind::Compaction(c) => Some(format!("[compaction: {}]", c.summary)),
                    SessionEntryKind::BranchSummary(b) => Some(format!("[branch: {}]", b.summary)),
                    _ => None,
                };
                text.map(|t| format!("{}: {}", role.unwrap_or("unknown"), t))
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        if !conversation.is_empty() {
            ctx.tui.chat.add_system_message("Generating branch summary...");
            let _ = ctx.tui.render_with_terminal(&mut ctx.terminal_manager);

            let system_prompt = "Summarize the following conversation branch concisely. Focus on key decisions, tool usage, and outcomes.";
            let context = pick_ai::Context {
                system_prompt: Some(system_prompt.to_string()),
                messages: vec![Message::User(UserMessage::text(&conversation))],
                tools: None,
            };
            let api_key = std::env::var(format!(
                "{}_API_KEY",
                ctx.provider.to_uppercase().replace('-', "_")
            ))
            .ok();
            let result =
                pick_ai::complete_simple(&ctx.model, context, api_key, None, Some(4096), None, None)
                    .await;

            if let Some(err) = &result.error_message {
                ctx.tui.show_error(&format!("Summarization failed: {}", err));
            } else {
                let summary: String = result
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
                if !summary.is_empty() {
                    let summary_entry = SessionEntry {
                        id: uuid::Uuid::now_v7().to_string(),
                        parent_id: Some(ancestor_id.clone()),
                        timestamp: chrono::Utc::now().timestamp_millis(),
                        kind: SessionEntryKind::BranchSummary(BranchSummaryEntry {
                            summary: summary.clone(),
                        }),
                    };
                    if let Err(e) = ctx.session_manager.append(summary_entry).await {
                        ctx.tui
                            .show_error(&format!("Failed to persist summary: {}", e));
                    }
                    ctx.tui.chat.add_system_message(&format!(
                        "Branch summary generated ({} chars).",
                        summary.len()
                    ));
                } else {
                    ctx.tui.chat.add_system_message("Empty summary. Navigating.");
                }
            }
        }
    }
    navigate_to(ctx, target_id);
}

/// Navigate to a session tree node (sync version with block_on for async append)
pub(crate) fn navigate_to(ctx: &mut TuiContext, target_id: &str) {
    let old_leaf = ctx.session_manager.get_leaf_id().map(|s| s.to_string());
    ctx.session_manager.set_leaf_id(target_id);

    let old = old_leaf.clone();
    let tid = target_id.to_string();
    let sm = &mut ctx.session_manager;
    let tui = &mut ctx.tui;
    let rt = tokio::runtime::Handle::current();
    let _ = rt.block_on(async move {
        if let Err(e) = sm.append_leaf_change(old, &tid).await {
            tui.show_error(&format!("Failed to record leaf change: {}", e));
        }
    });

    let path = ctx.session_manager.get_path_to_root(target_id);
    let new_messages: Vec<Message> = path
        .iter()
        .filter_map(|entry| Message::try_from(*entry).ok())
        .collect();
    ctx.all_messages = new_messages;
    ctx.tui.chat.clear();
    ctx.tui
        .chat
        .add_system_message(&format!(
            "Navigated. Context rebuilt with {} messages.",
            ctx.all_messages.len()
        ));
    for msg in &ctx.all_messages.clone() {
        if let Message::User(u) = msg {
            for block in &u.content {
                if let ContentBlock::Text(t) = block {
                    ctx.tui.chat.add_user_message(&t.text);
                }
            }
        }
    }
}
