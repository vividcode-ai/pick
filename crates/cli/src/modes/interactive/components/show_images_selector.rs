//! Show images selector component


use crate::core::tools::render_utils::ToolTheme;

/// Render show images selector with current value
pub fn render_show_images_selector(
    _current_value: bool,
    selected_index: usize,
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));

    let options = ["yes", "no"];
    let descriptions = ["Show images inline in terminal", "Show text placeholder instead"];

    lines.push(ToolTheme::fg("accent", &border));
    lines.push(String::new());

    for (i, option) in options.iter().enumerate() {
        let is_selected = i == selected_index;
        let prefix = if is_selected { "→" } else { " " };
        let desc = descriptions.get(i).unwrap_or(&"");
        let line = if is_selected {
            ToolTheme::fg("accent", &format!("  {} {}", prefix, option))
        } else {
            format!("    {} {}", prefix, option)
        };
        lines.push(format!("{}  {}", line, ToolTheme::fg("muted", desc)));
    }

    lines.push(String::new());
    lines.push(ToolTheme::fg("accent", &border));
    lines
}
