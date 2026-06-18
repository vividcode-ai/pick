//! Extension selector component for string list selection


use crate::core::tools::render_utils::ToolTheme;

/// Render extension selector with title and option list
pub fn render_extension_selector(
    title: &str,
    options: &[String],
    selected_index: usize,
    remaining_seconds: Option<u64>,
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));
    lines.push(ToolTheme::fg("accent", &border));
    lines.push(String::new());

    // Title with optional countdown
    let title_display = if let Some(secs) = remaining_seconds {
        format!("\x1b[1m{}\x1b[22m ({}s)", title, secs)
    } else {
        format!("\x1b[1m{}\x1b[22m", title)
    };
    lines.push(ToolTheme::fg("accent", &title_display));
    lines.push(String::new());

    // Options
    let max_visible = 10;
    let total = options.len();
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
        if let Some(option) = options.get(i) {
            let is_selected = i == selected_index;
            if is_selected {
                lines.push(ToolTheme::fg("accent", &format!("  → {}", option)));
            } else {
                lines.push(format!("    {}", option));
            }
        }
    }

    if total > max_visible {
        lines.push(ToolTheme::fg("muted", &format!("  ({}/{})", selected_index + 1, total)));
    }

    lines.push(String::new());
    lines.push(ToolTheme::fg("dim", "  ↑↓: navigate · Enter: select · Esc: cancel"));
    lines.push(String::new());
    lines.push(ToolTheme::fg("accent", &border));
    lines
}
