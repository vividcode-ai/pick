use pick_agent::extensions::types::{
    ExtensionEvent, SessionBeforeCompactEvent, SessionCompactEvent,
};
use pick_agent::session::{CompactionEntry, SessionEntry, SessionEntryKind};
use pick_ai::types::{ContentBlock, Message, UserMessage};
use pick_tui::app::TreeViewItem;
use pick_tui::components::select::{SelectItem, SelectList};

use super::context::TuiContext;
use super::init;
use super::tree_utils;

/// Handle /fork slash command: show fork point selection
pub(crate) fn handle_fork_selector(ctx: &mut TuiContext) {
    let user_msgs: Vec<(usize, String)> = ctx
        .all_messages
        .iter()
        .enumerate()
        .filter_map(|(i, msg)| {
            if let Message::User(u) = msg {
                let text: String = u
                    .content
                    .iter()
                    .filter_map(|b| {
                        if let ContentBlock::Text(t) = b {
                            Some(t.text.as_str())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<&str>>()
                    .join(" ");
                let text = text.trim().to_string();
                if !text.is_empty() {
                    Some((i, text))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    if user_msgs.is_empty() {
        ctx.tui
            .chat
            .add_system_message("No user messages to fork from.");
    } else {
        ctx.pending_command = Some("fork".to_string());
        let items: Vec<SelectItem> = user_msgs
            .iter()
            .map(|(i, text)| {
                let display = if text.len() > 60 {
                    format!("{}...", &text[..60])
                } else {
                    text.clone()
                };
                SelectItem::new(display, i.to_string()).with_description("Fork from this message")
            })
            .collect();
        let select = SelectList::new("Select message to fork from", items);
        ctx.tui.start_selection(select);
        ctx.tui.finalize_turn();
    }
}

/// Handle /tree slash command
pub(crate) fn handle_tree_command(ctx: &mut TuiContext, args: &[String]) {
    let tree_data = ctx.session_manager.build_tree();
    if tree_data.is_empty() {
        ctx.tui
            .chat
            .add_system_message("No entries in session to display.");
        ctx.tui.finalize_turn();
        return;
    }

    let filter_arg = args
        .first()
        .cloned()
        .unwrap_or_else(|| "default".to_string());
    let active_path: Vec<String> = ctx
        .session_manager
        .compute_active_path()
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    let mut items: Vec<TreeViewItem> = Vec::new();
    for node in &tree_data {
        let kind_str = tree_utils::entry_kind_str(&node.entry);
        let display_label = tree_utils::entry_label(&node.entry);
        let searchable = format!(
            "{} {} {}",
            kind_str,
            node.label.as_deref().unwrap_or(""),
            display_label
        );
        items.push(TreeViewItem {
            entry_id: node.entry_id.clone(),
            parent_id: node.parent_id.clone(),
            depth: node.depth,
            has_children: node.has_children,
            is_last: node.is_last,
            gutters: node.gutters.clone(),
            label: node.label.clone(),
            label_timestamp: node.label_timestamp.clone(),
            kind_str,
            searchable_text: searchable,
            display_label,
        });
    }

    if items.is_empty() {
        ctx.tui.chat.add_system_message("No entries to display.");
        ctx.tui.finalize_turn();
        return;
    }

    let current_leaf = ctx.session_manager.get_leaf_id().map(|s| s.to_string());
    let mut tree_view = pick_tui::app::TreeView::new(items, current_leaf, active_path);
    match filter_arg.as_str() {
        "no-tools" | "notools" => tree_view.set_filter_mode(pick_tui::app::TreeFilterMode::NoTools),
        "user-only" | "user" => tree_view.set_filter_mode(pick_tui::app::TreeFilterMode::UserOnly),
        "labeled-only" | "labeled" => {
            tree_view.set_filter_mode(pick_tui::app::TreeFilterMode::LabeledOnly)
        }
        "all" => tree_view.set_filter_mode(pick_tui::app::TreeFilterMode::All),
        _ => {}
    }
    ctx.tui.start_tree_view(tree_view);
    ctx.tui.finalize_turn();
}

/// Recursively scan a directory for .jsonl session files.
fn collect_session_files(dir: &std::path::Path, files: &mut Vec<(String, std::path::PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_session_files(&path, files);
        } else if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            if !files.iter().any(|(n, _)| n == &name) {
                files.push((name, path));
            }
        }
    }
}

/// Handle /resume slash command: show session list
pub(crate) fn handle_resume_selector(ctx: &mut TuiContext) {
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut session_files: Vec<(String, std::path::PathBuf)> = Vec::new();

    // Scan project directory
    let project_session_dir = cwd.join(".pick").join("sessions");
    if project_session_dir.exists() {
        collect_session_files(&project_session_dir, &mut session_files);
    }

    // Scan global sessions directory (~/.pick/agent/sessions/)
    if let Some(global) = dirs::home_dir().map(|h| h.join(".pick").join("agent").join("sessions")) {
        if global.exists() {
            collect_session_files(&global, &mut session_files);
        }
    }

    if session_files.is_empty() {
        ctx.tui.chat.add_system_message("No saved sessions found.");
    } else {
        ctx.pending_command = Some("resume".to_string());
        let items: Vec<SelectItem> = session_files
            .iter()
            .map(|(name, _)| {
                SelectItem::new(name.clone(), name.clone()).with_description("Resume this session")
            })
            .collect();
        let select = SelectList::new("Select session to resume", items);
        ctx.tui.start_selection(select);
        ctx.tui.finalize_turn();
    }
}

/// Handle /compact slash command
pub(crate) async fn handle_compact(ctx: &mut TuiContext, args: &[String]) {
    let msg_count = ctx.all_messages.len();
    if msg_count < 2 {
        ctx.tui
            .chat
            .add_system_message("Nothing to compact (need at least 2 messages).");
        ctx.tui.finalize_turn();
        return;
    }

    let custom_instructions = if args.is_empty() {
        None
    } else {
        Some(args.join(" "))
    };

    ctx.tui.chat.add_system_message(&format!(
        "Compacting \x1b[1m{}\x1b[0m messages...",
        msg_count
    ));
    let _ = ctx.tui.render_with_terminal(&mut ctx.terminal_manager);

    let path_entries: Vec<serde_json::Value> = ctx
        .all_messages
        .iter()
        .map(|msg| {
            let id = uuid::Uuid::now_v7().to_string();
            let message_val = match msg {
                Message::User(u) => serde_json::json!({"role": "user", "content": u.content}),
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
                    }
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

    use crate::core::compaction::compaction::{CompactionSettings, compact, prepare_compaction};
    let compact_settings = CompactionSettings::default();

    if let Some(ref runner) = ctx.extension_runner {
        runner.emit(&ExtensionEvent::SessionBeforeCompact(
            SessionBeforeCompactEvent {
                preparation: serde_json::json!({}),
                branch_entries: path_entries.clone(),
                custom_instructions: custom_instructions.clone(),
            },
        ));
    }

    let api_key = ctx
        .auth
        .get_api_key(&ctx.provider, true)
        .await
        .unwrap_or_default();

    match prepare_compaction(&path_entries, &compact_settings) {
        Some(preparation) => {
            match compact(
                &preparation,
                &ctx.model,
                &api_key,
                None,
                custom_instructions.as_deref(),
                None,
            )
            .await
            {
                Ok(compaction_result) => {
                    let summary = compaction_result.summary;
                    ctx.all_messages = vec![Message::User(UserMessage::text(format!(
                        "[Compacted conversation summary]\n\n{}",
                        summary
                    )))];
                    ctx.tui.chat.add_system_message(&format!(
                        "Compacted \x1b[1m{}\x1b[0m messages via compaction pipeline (\x1b[2m{} chars, {} tokens before\x1b[0m).",
                        msg_count,
                        summary.len(),
                        compaction_result.tokens_before
                    ));
                    let compact_entry = SessionEntry {
                        id: uuid::Uuid::now_v7().to_string(),
                        parent_id: None,
                        timestamp: chrono::Utc::now().timestamp_millis(),
                        kind: SessionEntryKind::Compaction(CompactionEntry {
                            summary: summary.clone(),
                            token_count: Some(compaction_result.tokens_before as u64),
                        }),
                    };
                    if let Err(e) = ctx.session_manager.append(compact_entry).await {
                        tracing::warn!("Failed to persist compaction entry: {}", e);
                    }
                    if let Some(ref runner) = ctx.extension_runner {
                        runner.emit(&ExtensionEvent::SessionCompact(SessionCompactEvent {
                            compaction_entry: serde_json::json!({
                                "summary": summary,
                                "tokensBefore": compaction_result.tokens_before,
                            }),
                            from_extension: false,
                        }));
                    }
                }
                Err(e) => {
                    ctx.tui
                        .show_error(&format!("Compaction pipeline failed: {}", e.message));
                }
            }
        }
        None => {
            ctx.tui
                .chat
                .add_system_message("Prepare returned None, using direct summarization...");
            use crate::core::compaction::compaction::generate_summary;
            let message_values: Vec<serde_json::Value> = ctx
                .all_messages
                .iter()
                .map(|m| serde_json::to_value(m).unwrap_or_default())
                .collect();
            match generate_summary(
                &message_values,
                &ctx.model,
                16384,
                &api_key,
                None,
                None,
                None,
                None,
            )
            .await
            {
                Ok(summary) => {
                    ctx.all_messages = vec![Message::User(UserMessage::text(format!(
                        "[Compacted conversation summary]\n\n{}",
                        summary
                    )))];
                    ctx.tui.chat.add_system_message(&format!(
                        "Compacted \x1b[1m{}\x1b[0m messages via direct summarization (\x1b[2m{} chars\x1b[0m).",
                        msg_count, summary.len()
                    ));
                    let compact_entry = SessionEntry {
                        id: uuid::Uuid::now_v7().to_string(),
                        parent_id: None,
                        timestamp: chrono::Utc::now().timestamp_millis(),
                        kind: SessionEntryKind::Compaction(CompactionEntry {
                            summary: summary.clone(),
                            token_count: None,
                        }),
                    };
                    if let Err(e) = ctx.session_manager.append(compact_entry).await {
                        tracing::warn!("Failed to persist compaction entry: {}", e);
                    }
                    if let Some(ref runner) = ctx.extension_runner {
                        runner.emit(&ExtensionEvent::SessionCompact(SessionCompactEvent {
                            compaction_entry: serde_json::json!({"summary": summary}),
                            from_extension: false,
                        }));
                    }
                }
                Err(e) => {
                    ctx.tui
                        .show_error(&format!("Compaction failed: {}", e.message));
                }
            }
        }
    }
}

/// Handle /reload slash command
pub(crate) async fn handle_reload(ctx: &mut TuiContext) {
    use crate::core::resource_loader::ResourceLoaderOptions;
    use crate::core::settings::SettingsManager;
    let sm = SettingsManager::load(&ctx.cwd);

    ctx.resource_loader
        .reload_with_options(
            &ctx.args.extensions,
            &ResourceLoaderOptions {
                no_skills: ctx.args.no_skills,
                no_themes: ctx.args.no_themes,
                no_context_files: ctx.args.no_context_files,
                theme_paths: ctx
                    .args
                    .themes
                    .iter()
                    .map(std::path::PathBuf::from)
                    .collect(),
            },
        )
        .await;
    let context_files = super::types::build_context_display_names(&ctx.resource_loader);
    let skills = super::types::build_skill_display_names(&ctx.resource_loader);

    let settings = sm.get();
    if let Some(new_provider) = &settings.default_provider
        && new_provider != &ctx.provider
    {
        ctx.provider = new_provider.clone();
        init::update_api_key(&ctx.auth, &ctx.provider).await;
    }
    if let Some(new_model) = &settings.default_model
        && new_model != &ctx.model_id
    {
        let (new_m, new_p) = init::update_model(&ctx.provider, new_model);
        ctx.model = new_m;
        ctx.model_id = new_model.clone();
        ctx.provider = new_p;
        ctx.tui.model_id = ctx.model_id.clone();
        ctx.tui.provider = ctx.provider.clone();
        init::update_api_key(&ctx.auth, &ctx.provider).await;
    }
    if let Some(new_think_level) = &settings.default_thinking_level {
        ctx.thinking_level = match new_think_level.as_str() {
            "low" => pick_agent::core::state::ThinkingLevel::Low,
            "medium" => pick_agent::core::state::ThinkingLevel::Medium,
            "high" => pick_agent::core::state::ThinkingLevel::High,
            _ => pick_agent::core::state::ThinkingLevel::Off,
        };
        ctx.tui.thinking_level = new_think_level.clone();
    }
    if let Some(new_theme) = &settings.theme {
        ctx.tui.chat.add_system_message(&format!(
            "Theme changed to \x1b[1m{}\x1b[0m (requires restart).",
            new_theme
        ));
    }
    if let Some(auto_compact) = settings.compaction.as_ref().and_then(|c| c.enabled) {
        ctx.tui.auto_compact = auto_compact;
    }

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

    ctx.tui
        .chat
        .add_system_message("Reloaded keybindings, extensions, skills, and themes.");
    if !context_files.is_empty() {
        ctx.tui.chat.add_system_message(&format!(
            "Context files: \x1b[2m{}\x1b[0m",
            context_files.join(", ")
        ));
    }
    if !skills.is_empty() {
        ctx.tui
            .chat
            .add_system_message(&format!("Skills: \x1b[2m{}\x1b[0m", skills.join(", ")));
    }
    let think_display = format!("{:?}", ctx.thinking_level).to_lowercase();
    ctx.tui.chat.add_system_message(&format!(
        "Provider: \x1b[1m{}\x1b[0m  Model: \x1b[1m{}\x1b[0m  Thinking: \x1b[2m{}\x1b[0m",
        ctx.provider, ctx.model_id, think_display
    ));
}
