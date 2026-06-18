//! Assistant message display


use crate::core::tools::render_utils::ToolTheme;

/// Render an assistant message with optional thinking block
pub fn render_assistant_message(
    text: &str,
    thinking_text: Option<&str>,
    hide_thinking: bool,
) -> String {
    let mut output = String::new();

    // Thinking block
    if let Some(thinking) = thinking_text {
        if !hide_thinking && !thinking.is_empty() {
            output.push_str(&ToolTheme::fg("dim", &format!("[thinking]\n{}\n[/thinking]\n", thinking)));
        }
    }

    // Main content
    if !text.is_empty() {
        output.push_str(text);
    }

    output
}
