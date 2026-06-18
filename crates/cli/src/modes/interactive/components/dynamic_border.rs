//! Dynamic border component that adjusts to viewport width


use crate::core::tools::render_utils::ToolTheme;

/// Render a horizontal border line across the full width
pub fn render_border(width: usize) -> String {
    ToolTheme::fg("dim", &"─".repeat(std::cmp::max(1, width)))
}

/// Render a colored horizontal border line
pub fn render_colored_border(width: usize, color: &str) -> String {
    ToolTheme::fg(color, &"─".repeat(std::cmp::max(1, width)))
}
