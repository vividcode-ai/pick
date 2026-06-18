//! Custom editor component with keybinding handler pattern

/// Render a custom editor display with keybinding hints
pub fn render_custom_editor(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));
    lines.push(border.clone());
    if text.is_empty() {
        lines.push(String::new());
    } else {
        for line in text.lines() {
            let clipped = if line.len() > width.saturating_sub(4) {
                &line[..width.saturating_sub(4)]
            } else {
                line
            };
            lines.push(format!("  {}", clipped));
        }
    }
    lines.push(border);
    lines
}
