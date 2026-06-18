//! Custom message component for extension-rendered messages


use crate::core::tools::render_utils::ToolTheme;

/// Render a custom message with label and content
pub fn render_custom_message(custom_type: &str, content: &str, _expanded: bool) -> String {
    let label = ToolTheme::fg("customMessageLabel", &format!("\x1b[1m[{}]\x1b[22m", custom_type));
    let text = ToolTheme::fg("customMessageText", content);
    format!("{}\n{}", label, text)
}
