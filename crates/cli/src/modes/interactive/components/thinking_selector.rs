//! Thinking level selector component

use crate::core::tools::render_utils::ToolTheme;

/// Thinking level descriptions
pub const LEVEL_DESCRIPTIONS: &[(&str, &str)] = &[
    ("off", "No reasoning"),
    ("minimal", "Very brief reasoning (~1k tokens)"),
    ("low", "Light reasoning (~2k tokens)"),
    ("medium", "Moderate reasoning (~8k tokens)"),
    ("high", "Deep reasoning (~16k tokens)"),
    ("xhigh", "Maximum reasoning (~32k tokens)"),
];

/// Render thinking level selector
pub fn render_thinking_selector(
    levels: &[String],
    current_level: &str,
    selected_index: usize,
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));

    lines.push(ToolTheme::fg("accent", &border));
    lines.push(String::new());

    let max_visible = 10;
    let total = levels.len();
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
        if let Some(level) = levels.get(i) {
            let is_selected = i == selected_index;
            let is_current = level == current_level;

            let desc = LEVEL_DESCRIPTIONS
                .iter()
                .find(|(k, _)| *k == level)
                .map(|(_, d)| *d)
                .unwrap_or("");

            let cursor = if is_selected { "→" } else { " " };
            let check = if is_current { " (current)" } else { "" };
            let line = if is_selected {
                format!(
                    "  {} {}",
                    ToolTheme::fg("accent", cursor),
                    ToolTheme::fg("accent", level)
                )
            } else {
                format!("    {} {}", cursor, level)
            };
            lines.push(format!(
                "{}{}  {}",
                line,
                ToolTheme::fg("muted", check),
                ToolTheme::fg("muted", desc)
            ));
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
