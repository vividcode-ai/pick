//! Theme selector component

use crate::core::tools::render_utils::ToolTheme;

/// Render theme selector with available themes
pub fn render_theme_selector(
    themes: &[String],
    current_theme: &str,
    selected_index: usize,
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));

    lines.push(ToolTheme::fg("accent", &border));
    lines.push(String::new());

    let max_visible = 10;
    let total = themes.len();
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
        if let Some(theme) = themes.get(i) {
            let is_selected = i == selected_index;
            let is_current = theme == current_theme;
            let cursor = if is_selected { "→" } else { " " };
            let check = if is_current { " (current)" } else { "" };
            let line = if is_selected {
                format!(
                    "  {} {}",
                    ToolTheme::fg("accent", cursor),
                    ToolTheme::fg("accent", theme)
                )
            } else {
                format!("    {} {}", cursor, theme)
            };
            lines.push(format!("{}{}", line, ToolTheme::fg("muted", check)));
        }
    }

    if total > max_visible {
        lines.push(ToolTheme::fg(
            "muted",
            &format!("  ({}/{})", selected_index + 1, total),
        ));
    }

    lines.push(String::new());
    lines.push(ToolTheme::fg("accent", &border));
    lines
}
