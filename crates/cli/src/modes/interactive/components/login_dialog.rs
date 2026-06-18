//! Login dialog component for OAuth login flow


use crate::core::tools::render_utils::ToolTheme;

/// Render the login dialog with provider info
pub fn render_login_dialog(
    title: &str,
    provider_name: &str,
    url: Option<&str>,
    user_code: Option<&str>,
    status_message: Option<&str>,
    instructions: Option<&str>,
    input_prompt: Option<&str>,
    input_value: &str,
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));
    lines.push(ToolTheme::fg("accent", &border));

    // Title
    lines.push(ToolTheme::fg("accent", &format!("\x1b[1m{}\x1b[22m", title)));
    lines.push(String::new());

    // Provider name
    lines.push(ToolTheme::fg("accent", &format!("Provider: {}", provider_name)));
    lines.push(String::new());

    // Auth URL
    if let Some(url) = url {
        lines.push(ToolTheme::fg("accent", url));
        lines.push(ToolTheme::fg("dim", "Ctrl+click to open"));
        lines.push(String::new());
    }

    // User code (device code flow)
    if let Some(code) = user_code {
        lines.push(ToolTheme::fg("warning", &format!("Enter code: {}", code)));
        lines.push(String::new());
    }

    // Instructions
    if let Some(instr) = instructions {
        lines.push(ToolTheme::fg("warning", instr));
        lines.push(String::new());
    }

    // Input prompt
    if let Some(prompt) = input_prompt {
        lines.push(ToolTheme::fg("text", prompt));
        let display_val = if input_value.is_empty() {
            ToolTheme::fg("muted", "(waiting for input)")
        } else {
            input_value.to_string()
        };
        lines.push(format!("  {}", display_val));
        lines.push(ToolTheme::fg("dim", "(Esc to cancel, Enter to submit)"));
        lines.push(String::new());
    }

    // Status message
    if let Some(msg) = status_message {
        lines.push(ToolTheme::fg("dim", msg));
        lines.push(String::new());
    }

    lines.push(ToolTheme::fg("dim", "(Esc to cancel)"));
    lines.push(ToolTheme::fg("accent", &border));
    lines
}

/// Render login progress message
pub fn render_login_progress(message: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));
    lines.push(ToolTheme::fg("accent", &border));
    lines.push(ToolTheme::fg("dim", message));
    lines.push(ToolTheme::fg("dim", "(Esc to cancel)"));
    lines.push(ToolTheme::fg("accent", &border));
    lines
}

/// Render login info display
pub fn render_login_info(lines_in: &[String], width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));
    lines.push(ToolTheme::fg("accent", &border));
    lines.push(String::new());
    for line in lines_in {
        lines.push(line.clone());
    }
    lines.push(String::new());
    lines.push(ToolTheme::fg("dim", "(Esc to close)"));
    lines.push(ToolTheme::fg("accent", &border));
    lines
}
