//! Extension input component with countdown timer


use crate::core::tools::render_utils::ToolTheme;

/// Render extension input with title and value
pub fn render_extension_input(
    title: &str,
    value: &str,
    remaining_seconds: Option<u64>,
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));
    lines.push(ToolTheme::fg("accent", &border));
    lines.push(String::new());

    // Title with optional countdown
    let title_display = if let Some(secs) = remaining_seconds {
        format!("{} ({}s)", title, secs)
    } else {
        title.to_string()
    };
    lines.push(ToolTheme::fg("accent", &format!("\x1b[1m{}\x1b[22m", title_display)));

    lines.push(String::new());
    // Input value
    let display_value = if value.is_empty() {
        ToolTheme::fg("muted", "  (empty)")
    } else {
        let clipped = if value.len() > width.saturating_sub(4) {
            format!("{}...", &value[..width.saturating_sub(7)])
        } else {
            value.to_string()
        };
        format!("  {}", clipped)
    };
    lines.push(display_value);

    lines.push(String::new());
    lines.push(ToolTheme::fg("dim", "  Enter: submit · Esc: cancel"));
    lines.push(String::new());
    lines.push(ToolTheme::fg("accent", &border));
    lines
}
