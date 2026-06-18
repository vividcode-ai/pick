use pick_agent::session::{SessionEntry, SessionManager};
use pick_ai::types::{ContentBlock, Message};
use pick_tui::app::{TreeView, TreeViewItem};
use pick_tui::components::select::{SelectItem, SelectList};

use super::context::TuiContext;
use super::message_utils;
use super::tree_utils;

/// Handle OpenTree action: build and show session tree view
pub(crate) async fn handle_open_tree(ctx: &mut TuiContext) {
    let tree_data = ctx.session_manager.build_tree();
    if tree_data.is_empty() {
        ctx.tui
            .chat
            .add_system_message("No entries in session to display.");
        ctx.tui.finalize_turn();
        return;
    }

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

    let current_leaf = ctx
        .session_manager
        .get_leaf_id()
        .map(|s| s.to_string());
    let tree_view = TreeView::new(items, current_leaf, active_path);
    ctx.tui.start_tree_view(tree_view);
    ctx.tui.finalize_turn();
}

/// Handle tree selection result (navigation, labeling)
pub(crate) async fn handle_tree_selection(
    ctx: &mut TuiContext,
    val: &str,
) -> Option<String> {
    // Label change
    if let Some(rest) = val.strip_prefix("__label__") {
        if let Some(delim) = rest.find(':') {
            let entry_id = &rest[..delim];
            let label = &rest[delim + 1..];
            let label_opt = if label.is_empty() {
                None
            } else {
                Some(label.to_string())
            };
            if let Err(e) = ctx
                .session_manager
                .append_label_change(entry_id, label_opt.as_deref())
                .await
            {
                ctx.tui
                    .show_error(&format!("Failed to save label: {}", e));
            } else {
                ctx.tui.chat.add_system_message(&format!(
                    "Label {} for entry.",
                    if label_opt.is_some() {
                        "set"
                    } else {
                        "cleared"
                    }
                ));
            }
        }
        return Some(String::new());
    }

    // Regular navigation
    let target_id = val.to_string();
    let current_leaf = ctx.session_manager.get_leaf_id();
    if current_leaf == Some(target_id.as_str()) {
        ctx.tui
            .chat
            .add_system_message("Already at this point.");
        return Some(String::new());
    }

    // Check if summarization is needed
    let has_entries = current_leaf.is_some()
        && ctx
            .session_manager
            .find_common_ancestor(current_leaf.unwrap(), &target_id)
            .is_some();
    if has_entries {
        let target_id2 = target_id.clone();
        ctx.pending_command = Some(format!("tree-summarize:{}", target_id2));
        let items = vec![
            SelectItem::new("No summary", "no-summary")
                .with_description("Navigate without summarization"),
            SelectItem::new("Summarize", "summarize")
                .with_description("Generate LLM summary of abandoned branch"),
        ];
        let select = SelectList::new("Summarize branch?", items);
        ctx.tui.start_selection(select);
        ctx.tui.finalize_turn();
        return None; // Don't clear pending_command yet
    }

    // Navigate directly
    let old_leaf = current_leaf.map(|s| s.to_string());
    ctx.session_manager.set_leaf_id(&target_id);
    if let Err(e) = ctx
        .session_manager
        .append_leaf_change(old_leaf, &target_id)
        .await
    {
        ctx.tui
            .show_error(&format!("Failed to record leaf change: {}", e));
    }
    rebuild_session_after_navigation(ctx, &target_id);
    Some(String::new())
}

/// Handle tree summarization result
/// Handle fork session
pub(crate) async fn handle_fork(ctx: &mut TuiContext, idx: usize) {
    let cwd = std::env::current_dir().unwrap_or_default();
    let session_dir = cwd.join(".pick").join("sessions");

    // Fire session_before_tree extension event
    if let Some(ref runner) = ctx.extension_runner {
        use pick_agent::extensions::types::{
            ExtensionEvent, SessionBeforeTreeEvent, TreePreparation,
        };
        runner.emit(&ExtensionEvent::SessionBeforeTree(SessionBeforeTreeEvent {
            preparation: TreePreparation {
                target_id: uuid::Uuid::now_v7().to_string(),
                old_leaf_id: None,
                common_ancestor_id: None,
                entries_to_summarize: Vec::new(),
                user_wants_summary: false,
                custom_instructions: None,
                replace_instructions: None,
                label: None,
            },
        }));
    }

    match SessionManager::create(cwd.clone(), Some(session_dir)).await {
        Ok(mut new_mgr) => {
            let fork_msgs: Vec<Message> = ctx.all_messages
                [..=idx.min(ctx.all_messages.len().saturating_sub(1))]
                .to_vec();
            for msg in &fork_msgs {
                if let Err(e) = new_mgr.append(SessionEntry::from(msg)).await {
                    ctx.tui
                        .show_error(&format!("Fork persist failed: {}", e));
                }
            }
            ctx.all_messages = fork_msgs;
            ctx.session_manager = new_mgr;
            ctx.tui.session_name = None;
            ctx.tui.update_terminal_title();
            ctx.tui.chat.add_system_message(&format!(
                "Forked new session with \x1b[1m{}\x1b[0m messages.",
                ctx.all_messages.len()
            ));

            // Fire session_tree extension event
            if let Some(ref runner) = ctx.extension_runner {
                use pick_agent::extensions::types::{
                    ExtensionEvent, SessionTreeEvent,
                };
                runner.emit(&ExtensionEvent::SessionTree(SessionTreeEvent {
                    new_leaf_id: Some(uuid::Uuid::now_v7().to_string()),
                    old_leaf_id: None,
                    summary_entry: None,
                    from_extension: Some(false),
                }));
            }
        }
        Err(e) => ctx
            .tui
            .show_error(&format!("Fork failed: {}", e)),
    }
}

/// Handle resume session
pub(crate) async fn handle_resume(ctx: &mut TuiContext, session_id: &str) {
    let cwd = std::env::current_dir().unwrap_or_default();
    let session_dir = cwd.join(".pick").join("sessions");
    let session_path = session_dir.join(format!("{}.jsonl", session_id));

    let session_path = if session_path.exists() {
        session_path
    } else if let Some(global) = dirs::home_dir().map(|h| h.join(".pick").join("sessions")) {
        let global_path = global.join(format!("{}.jsonl", session_id));
        if global_path.exists() {
            global_path
        } else {
            session_path
        }
    } else {
        session_path
    };

    match SessionManager::open(session_path, cwd.clone()).await {
        Ok(new_mgr) => {
            let entries = new_mgr.entries().to_vec();
            let msgs = message_utils::entries_to_messages(&entries);
            ctx.all_messages = msgs;
            ctx.session_manager = new_mgr;
            if let Some(name) = ctx.session_manager.get_session_name() {
                ctx.tui.set_session_name(name.to_string());
            }
            ctx.tui.chat.add_system_message(&format!(
                "Resumed session with \x1b[1m{}\x1b[0m messages.",
                ctx.all_messages.len()
            ));
        }
        Err(e) => ctx
            .tui
            .show_error(&format!("Failed to resume session: {}", e)),
    }
}

/// Handle clone session
pub(crate) async fn handle_clone(ctx: &mut TuiContext) {
    if ctx.all_messages.is_empty() {
        ctx.tui
            .chat
            .add_system_message("Nothing to clone yet.");
        return;
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let session_dir = cwd.join(".pick").join("sessions");
    match SessionManager::create(cwd.clone(), Some(session_dir)).await {
        Ok(mut new_mgr) => {
            let msg_count = ctx.all_messages.len();
            for msg in &ctx.all_messages {
                if let Err(e) = new_mgr.append(SessionEntry::from(msg)).await {
                    ctx.tui
                        .show_error(&format!("Clone persist failed: {}", e));
                }
            }
            ctx.session_manager = new_mgr;
            ctx.tui.session_name = None;
            ctx.tui.update_terminal_title();
            ctx.tui.chat.add_system_message(&format!(
                "Cloned session with \x1b[1m{}\x1b[0m messages.",
                msg_count
            ));
        }
        Err(e) => ctx
            .tui
            .show_error(&format!("Clone failed: {}", e)),
    }
}

/// Handle new session
pub(crate) async fn handle_new_session(ctx: &mut TuiContext) {
    let cwd = std::env::current_dir().unwrap_or_default();
    let session_dir = cwd.join(".pick").join("sessions");
    let width = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80);

    match SessionManager::create(cwd, Some(session_dir)).await {
        Ok(new_mgr) => {
            ctx.all_messages.clear();
            ctx.tui.chat.clear();
            ctx.tui.reset_scrollback_state();
            let _ = ctx
                .terminal_manager
                .terminal_mut()
                .clear_scrollback_and_visible_screen_ansi();
            let _ = ctx.terminal_manager.reset_for_new_session();
            ctx.tui.show_startup_header(width);
            ctx.session_manager = new_mgr;
            ctx.tui.session_name = None;
            ctx.tui.update_terminal_title();
        }
        Err(e) => {
            ctx.all_messages.clear();
            ctx.tui.chat.clear();
            ctx.tui.reset_scrollback_state();
            let _ = ctx
                .terminal_manager
                .terminal_mut()
                .clear_scrollback_and_visible_screen_ansi();
            let _ = ctx.terminal_manager.reset_for_new_session();
            ctx.tui.show_startup_header(width);
            ctx.tui
                .show_error(&format!("Session create failed: {}", e));
            ctx.tui.session_name = None;
            ctx.tui.update_terminal_title();
        }
    }
}

fn rebuild_session_after_navigation(ctx: &mut TuiContext, _target_id: &str) {
    // This is called from handle_tree_selection when navigating directly.
    // navigate_to does all the work including the async leaf change.
    // But since we handle async separately, we call the sync parts directly.
    let tid = ctx.session_manager.get_leaf_id().unwrap_or_default().to_string();

    let path = ctx.session_manager.get_path_to_root(&tid);
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
    for msg in &ctx.all_messages {
        if let Message::User(u) = msg {
            for block in &u.content {
                if let ContentBlock::Text(t) = block {
                    ctx.tui.chat.add_user_message(&t.text);
                }
            }
        }
    }
}
