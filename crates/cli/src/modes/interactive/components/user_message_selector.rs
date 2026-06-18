//! User message selector component for session branching


use crate::core::tools::render_utils::ToolTheme;

/// A user message item for display
#[derive(Clone)]
pub struct UserMessageItem {
    pub id: String,
    pub text: String,
    pub timestamp: Option<String>,
}

/// Render user message selector for forking
pub fn render_user_message_selector(
    messages: &[UserMessageItem],
    selected_index: usize,
    _initial_selected_id: Option<&str>,
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));

    // Header
    lines.push(String::new());
    lines.push(ToolTheme::bold("Fork from Message"));
    lines.push(ToolTheme::fg("muted", "Select a user message to copy the active path up to that point into a new session"));
    lines.push(String::new());
    lines.push(ToolTheme::fg("accent", &border));
    lines.push(String::new());

    if messages.is_empty() {
        lines.push(ToolTheme::fg("muted", "  No user messages found"));
        lines.push(String::new());
        lines.push(ToolTheme::fg("accent", &border));
        return lines;
    }

    let max_visible = 10;
    let total = messages.len();
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
        if let Some(msg) = messages.get(i) {
            let is_selected = i == selected_index;
            let normalized = msg.text.replace('\n', " ").trim().to_string();
            let max_msg_width = width.saturating_sub(4);
            let truncated = if normalized.len() > max_msg_width {
                format!("{}...", &normalized[..max_msg_width.saturating_sub(3)])
            } else {
                normalized
            };

            let cursor = if is_selected {
                ToolTheme::fg("accent", "› ")
            } else {
                "  ".to_string()
            };

            let msg_line = if is_selected {
                ToolTheme::bold(&truncated)
            } else {
                truncated
            };
            lines.push(format!("{}{}", cursor, msg_line));

            // Metadata line
            let metadata = format!("  Message {} of {}", i + 1, total);
            lines.push(ToolTheme::fg("muted", &metadata));
            lines.push(String::new());
        }
    }

    if total > max_visible {
        lines.push(ToolTheme::fg("muted", &format!("  ({}/{})", selected_index + 1, total)));
    }

    lines.push(String::new());
    lines.push(ToolTheme::fg("accent", &border));
    lines
}
