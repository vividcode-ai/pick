//! Branch summary message display


use crate::core::tools::render_utils::ToolTheme;

/// Render a branch summary message
pub fn render_branch_summary(summary: &str) -> String {
    format!(
        "{}\n{}",
        ToolTheme::fg("warning", "[Branch Summary]"),
        summary
    )
}
