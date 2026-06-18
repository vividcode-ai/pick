//! Extension editor component with external editor support

use crate::core::tools::render_utils::ToolTheme;

/// Render extension editor with title, content, and key hints
pub fn render_extension_editor(
    title: &str,
    content: &str,
    has_external_editor: bool,
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));
    lines.push(ToolTheme::fg("accent", &border));
    lines.push(String::new());
    lines.push(ToolTheme::fg(
        "accent",
        &format!("\x1b[1m{}\x1b[22m", title),
    ));
    lines.push(String::new());

    // Content
    if content.is_empty() {
        lines.push(ToolTheme::fg("muted", "  (empty)"));
    } else {
        for line in content.lines() {
            let clipped = if line.len() > width.saturating_sub(4) {
                format!("{}...", &line[..width.saturating_sub(7)])
            } else {
                line.to_string()
            };
            lines.push(format!("  {}", clipped));
        }
    }

    lines.push(String::new());
    // Key hints
    let mut hints = vec!["Enter: submit", "Shift+Enter: newline", "Esc: cancel"];
    if has_external_editor {
        hints.push("Ctrl+G: external editor");
    }
    lines.push(ToolTheme::fg("dim", &format!("  {}", hints.join(" · "))));
    lines.push(String::new());
    lines.push(ToolTheme::fg("accent", &border));
    lines
}
