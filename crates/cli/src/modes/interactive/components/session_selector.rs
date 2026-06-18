//! Session selector component with tree view and search

use crate::core::tools::render_utils::ToolTheme;

/// Session info for display
#[derive(Debug, Clone)]
pub struct SessionDisplayInfo {
    pub path: String,
    pub name: Option<String>,
    pub first_message: Option<String>,
    pub message_count: u32,
    pub modified: String, // ISO8601
    pub cwd: Option<String>,
    pub parent_session_path: Option<String>,
    pub is_current: bool,
}

/// Flat session node for tree display
#[derive(Debug, Clone)]
pub struct FlatSessionNode {
    pub session: SessionDisplayInfo,
    pub depth: usize,
    pub is_last: bool,
    pub ancestor_continues: Vec<bool>,
}

fn shorten_path(path: &str) -> String {
    // Simple home dir shortening
    if let Some(rest) = path
        .strip_prefix("/home/")
        .or_else(|| path.strip_prefix("/Users/"))
    {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() > 1 {
            return format!("~{}/{}", parts[0], parts[1..].join("/"));
        }
        return format!("~{}", parts[0]);
    }
    if let Some(rest) = path
        .strip_prefix("C:\\Users\\")
        .or_else(|| path.strip_prefix("c:\\Users\\"))
    {
        let parts: Vec<&str> = rest.split('\\').collect();
        if parts.len() > 1 {
            return format!("~{}/{}", parts[0], parts[1..].join("/"));
        }
        return format!("~{}", parts[0]);
    }
    path.to_string()
}

fn format_session_date(_iso_timestamp: &str) -> String {
    // Return a simple relative time string from the stored value
    // The caller should provide already-formatted relative time strings
    if _iso_timestamp.is_empty() {
        "now".to_string()
    } else {
        _iso_timestamp.to_string()
    }
}

/// Build session tree from flat session list, returning flat nodes with tree structure.
pub fn build_session_tree(sessions: &[SessionDisplayInfo]) -> Vec<FlatSessionNode> {
    let mut children: Vec<Vec<usize>> = vec![vec![]; sessions.len()];
    let mut roots = Vec::new();

    for (i, session) in sessions.iter().enumerate() {
        let parent_path = session.parent_session_path.as_deref();
        let parent_idx = parent_path.and_then(|pp| sessions.iter().position(|s| s.path == pp));
        if let Some(idx) = parent_idx {
            children[idx].push(i);
        } else {
            roots.push(i);
        }
    }

    // Sort each children group by modified date (descending)
    for child_group in &mut children {
        child_group.sort_by(|&a, &b| {
            sessions[b]
                .modified
                .as_str()
                .cmp(sessions[a].modified.as_str())
        });
    }
    roots.sort_by(|&a, &b| {
        sessions[b]
            .modified
            .as_str()
            .cmp(sessions[a].modified.as_str())
    });

    // Flatten
    let mut result = Vec::new();
    fn walk(
        idx: usize,
        sessions: &[SessionDisplayInfo],
        children: &[Vec<usize>],
        depth: usize,
        ancestor_continues: &[bool],
        is_last: bool,
        result: &mut Vec<FlatSessionNode>,
    ) {
        result.push(FlatSessionNode {
            session: sessions[idx].clone(),
            depth,
            is_last,
            ancestor_continues: ancestor_continues.to_vec(),
        });
        let child_count = children[idx].len();
        for (ci, &child) in children[idx].iter().enumerate() {
            let child_is_last = ci == child_count - 1;
            let continues = if depth > 0 { !is_last } else { false };
            let mut child_ancestors = ancestor_continues.to_vec();
            child_ancestors.push(continues);
            walk(
                child,
                sessions,
                children,
                depth + 1,
                &child_ancestors,
                child_is_last,
                result,
            );
        }
    }

    for (i, &root) in roots.iter().enumerate() {
        walk(
            root,
            sessions,
            &children,
            0,
            &[],
            i == roots.len() - 1,
            &mut result,
        );
    }

    result
}

/// Render session selector header
pub fn render_session_selector_header(
    scope: &str,
    sort_mode: &str,
    name_filter: &str,
    loading: bool,
    width: usize,
) -> Vec<String> {
    let title = if scope == "current" {
        "Resume Session (Current Folder)"
    } else {
        "Resume Session (All)"
    };
    let left = ToolTheme::bold(title);

    let sort_label = match sort_mode {
        "threaded" => "Threaded",
        "recent" => "Recent",
        "relevance" => "Fuzzy",
        _ => sort_mode,
    };
    let sort_text = format!(
        "{}{}",
        ToolTheme::fg("muted", "Sort: "),
        ToolTheme::fg("accent", sort_label),
    );
    let name_label = if name_filter == "all" { "All" } else { "Named" };
    let name_text = format!(
        "{}{}",
        ToolTheme::fg("muted", "Name: "),
        ToolTheme::fg("accent", name_label),
    );

    let scope_text = if loading {
        format!(
            "{}{}",
            ToolTheme::fg("muted", "○ Current Folder | "),
            ToolTheme::fg("accent", "Loading..."),
        )
    } else if scope == "current" {
        format!(
            "{}{}",
            ToolTheme::fg("accent", "◉ Current Folder"),
            ToolTheme::fg("muted", " | ○ All"),
        )
    } else {
        format!(
            "{}{}",
            ToolTheme::fg("muted", "○ Current Folder | "),
            ToolTheme::fg("accent", "◉ All"),
        )
    };

    let right = format!("{}  {}  {}", scope_text, name_text, sort_text);
    let available_left = width.saturating_sub(right.len() + 1);
    let left_trunc = if left.len() > available_left {
        format!("{}...", &left[..available_left.saturating_sub(3)])
    } else {
        left
    };
    let spacing = width.saturating_sub(left_trunc.len() + right.len());

    // Hint lines
    let hint1 = format!(
        "{}Tab: scope{} · re:<pattern> regex · \"phrase\" exact",
        ToolTheme::fg("accent", ""),
        ToolTheme::fg("muted", ""),
    );
    let hint2 = format!(
        "{}Ctrl+T: sort{} · Ctrl+N: named{} · Ctrl+D: delete{} · Ctrl+P: path{}",
        ToolTheme::fg("accent", ""),
        ToolTheme::fg("muted", ""),
        ToolTheme::fg("muted", ""),
        ToolTheme::fg("muted", ""),
        ToolTheme::fg("muted", ""),
    );

    vec![
        format!("{}{}{}", left_trunc, " ".repeat(spacing), right),
        ToolTheme::fg("muted", &hint1),
        ToolTheme::fg("muted", &hint2),
    ]
}

/// Render session list with tree structure
pub fn render_session_list(
    sessions: &[FlatSessionNode],
    selected_index: usize,
    search_query: &str,
    show_cwd: bool,
    show_path: bool,
    confirming_delete_path: Option<&str>,
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();

    // Search input
    let search_display = if search_query.is_empty() {
        ToolTheme::fg("muted", "  Type to search...")
    } else {
        format!("  {}", search_query)
    };
    lines.push(search_display);
    lines.push(String::new());

    if sessions.is_empty() {
        lines.push(ToolTheme::fg(
            "muted",
            "  No sessions found. Press Tab to view all.",
        ));
        return lines;
    }

    let max_visible = 10;
    let total = sessions.len();
    let start = if total > max_visible {
        let half = max_visible / 2;
        if selected_index > half {
            std::cmp::min(selected_index - half, total - max_visible)
        } else {
            0
        }
    } else {
        0
    };
    let end = std::cmp::min(start + max_visible, total);

    for i in start..end {
        if let Some(node) = sessions.get(i) {
            let session = &node.session;
            let is_selected = i == selected_index;
            let is_confirming_delete = confirming_delete_path.map_or(false, |p| p == session.path);

            // Tree prefix
            let tree_prefix = build_tree_prefix(node);

            // Display text
            let has_name = session.name.is_some();
            let display_text = session
                .name
                .as_deref()
                .or(session.first_message.as_deref())
                .unwrap_or("(empty)");
            let normalized: String = display_text
                .chars()
                .filter(|&c| !c.is_control() || c == '\t')
                .collect();
            let normalized = normalized.trim();

            // Right part
            let age = format_session_date(&session.modified);
            let msg_count = session.message_count.to_string();
            let mut right = format!("{} {}", msg_count, age);
            if show_cwd {
                if let Some(ref cwd) = session.cwd {
                    right = format!("{} {}", shorten_path(cwd), right);
                }
            }
            if show_path {
                right = format!("{} {}", shorten_path(&session.path), right);
            }

            // Cursor
            let cursor = if is_selected {
                ToolTheme::fg("accent", "› ")
            } else {
                "  ".to_string()
            };

            let prefix_width = tree_prefix.len();
            let right_width = right.len() + 2;
            let available = width.saturating_sub(cursor.len() + prefix_width + right_width);
            let truncated = if normalized.len() > available {
                format!("{}…", &normalized[..available.saturating_sub(1)])
            } else {
                normalized.to_string()
            };

            // Style
            let styled_msg = if is_confirming_delete {
                ToolTheme::fg("error", &truncated)
            } else if session.is_current {
                ToolTheme::fg("accent", &truncated)
            } else if has_name {
                ToolTheme::fg("warning", &truncated)
            } else {
                truncated
            };
            let styled_msg = if is_selected {
                ToolTheme::bold(&styled_msg)
            } else {
                styled_msg
            };

            let left = format!(
                "{}{}{}",
                cursor,
                ToolTheme::fg("dim", &tree_prefix),
                styled_msg
            );
            let spacing = width.saturating_sub(left.len() + right.len());
            let styled_right = if is_confirming_delete {
                ToolTheme::fg("error", &right)
            } else {
                ToolTheme::fg("dim", &right)
            };
            let line = format!("{}{}{}", left, " ".repeat(spacing), styled_right);
            let line = if is_selected {
                format!("\x1b[48;5;236m{}\x1b[49m", line)
            } else {
                line
            };
            lines.push(line);
        }
    }

    if total > max_visible {
        lines.push(ToolTheme::fg(
            "muted",
            &format!("  ({}/{})", selected_index + 1, total),
        ));
    }

    lines
}

fn build_tree_prefix(node: &FlatSessionNode) -> String {
    if node.depth == 0 {
        return String::new();
    }

    let mut result = String::new();
    for &continues in &node.ancestor_continues {
        if continues {
            result.push_str("│  ");
        } else {
            result.push_str("   ");
        }
    }
    if node.is_last {
        result.push_str("└─ ");
    } else {
        result.push_str("├─ ");
    }
    result
}

/// Render rename mode panel
pub fn render_rename_panel(current_name: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));
    lines.push(ToolTheme::fg("accent", &border));
    lines.push(String::new());
    lines.push(ToolTheme::bold("Rename Session"));
    lines.push(String::new());
    let display = if current_name.is_empty() {
        ToolTheme::fg("muted", "(empty)")
    } else {
        current_name.to_string()
    };
    lines.push(format!("  {}", display));
    lines.push(String::new());
    lines.push(ToolTheme::fg("muted", "Enter: save · Esc: cancel"));
    lines.push(String::new());
    lines.push(ToolTheme::fg("accent", &border));
    lines
}
