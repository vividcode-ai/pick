//! Loader wrapped with borders for extension UI

use crate::core::tools::render_utils::ToolTheme;

/// Render a bordered loader with message
pub fn render_bordered_loader(message: &str, cancellable: bool, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));
    lines.push(ToolTheme::fg("dim", &border));
    lines.push(format!(
        " {} {}",
        ToolTheme::fg("accent", "\u{25D0}"),
        message
    ));
    if cancellable {
        lines.push(format!(
            " {} {}",
            ToolTheme::fg("muted", ""),
            ToolTheme::fg("dim", "(Esc to cancel)")
        ));
    }
    lines.push(ToolTheme::fg("dim", &border));
    lines
}
