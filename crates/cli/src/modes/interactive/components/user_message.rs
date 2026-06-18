//! User message display


use crate::core::tools::render_utils::ToolTheme;

/// Render a user message
pub fn render_user_message(text: &str) -> String {
    format!("{} {}", ToolTheme::fg("toolTitle", "You:"), text)
}
