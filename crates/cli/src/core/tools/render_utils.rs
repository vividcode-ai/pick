/// Shorten a path by replacing home directory with ~
pub fn shorten_path(path: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        if path.starts_with(home_str.as_ref()) {
            return format!("~{}", &path[home_str.len()..]);
        }
    }
    path.to_string()
}

/// Safely convert a value to string for display
pub fn str_val(value: Option<&str>) -> Option<String> {
    value.map(|s| s.to_string())
}

/// Replace tabs with spaces
pub fn replace_tabs(text: &str) -> String {
    text.replace('\t', "   ")
}

/// Normalize display text by removing CR characters
pub fn normalize_display_text(text: &str) -> String {
    text.replace('\r', "")
}

/// Format invalid argument text
pub fn invalid_arg_text<F: Fn(&str) -> String>(fg: &F) -> String {
    fg("[invalid arg]")
}

// ============================================================================
// Tool Rendering Types
// ============================================================================

/// Context passed to tool render functions
#[derive(Debug, Clone)]
pub struct ToolRenderContext {
    pub args: Option<serde_json::Value>,
    pub cwd: String,
    pub expanded: bool,
    pub show_images: bool,
    pub is_error: bool,
}

/// Options for rendering tool call results
#[derive(Debug, Clone)]
pub struct ToolRenderOptions {
    pub expanded: bool,
    pub is_partial: bool,
}

/// A formatted tool rendering result
#[derive(Debug, Clone)]
pub struct ToolRenderOutput {
    pub label: String,
    pub formatted: String,
}

/// Simple ANSI-based theme functions for tool rendering
/// This provides basic string formatting without depending on the full theme system.
pub struct ToolTheme;

impl ToolTheme {
    pub fn bold(text: &str) -> String {
        format!("\x1b[1m{}\x1b[22m", text)
    }

    pub fn fg(color: &str, text: &str) -> String {
        match color {
            "accent" => format!("\x1b[36m{}\x1b[39m", text),
            "toolTitle" => format!("\x1b[1;36m{}\x1b[22;39m", text),
            "toolOutput" => format!("\x1b[37m{}\x1b[39m", text),
            "warning" => format!("\x1b[33m{}\x1b[39m", text),
            "error" => format!("\x1b[31m{}\x1b[39m", text),
            "success" => format!("\x1b[32m{}\x1b[39m", text),
            "dim" => format!("\x1b[2m{}\x1b[22m", text),
            "muted" => format!("\x1b[90m{}\x1b[39m", text),
            "customMessageLabel" => format!("\x1b[35m{}\x1b[39m", text),
            "customMessageText" => format!("\x1b[37m{}\x1b[39m", text),
            _ => text.to_string(),
        }
    }
}

/// Extract text content from a tool result content array
pub fn get_text_output(content: &[serde_json::Value], _show_images: bool) -> String {
    let mut parts = Vec::new();
    for block in content {
        if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
            parts.push(text.to_string());
        }
    }
    parts.join("")
}
