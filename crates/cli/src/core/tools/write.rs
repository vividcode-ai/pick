use tokio::fs;
use std::path::Path;

use super::path_utils::resolve_to_cwd;
use super::render_utils::{ToolRenderContext, ToolRenderOptions, ToolRenderOutput, ToolTheme,
    shorten_path, invalid_arg_text, normalize_display_text, replace_tabs};

/// Create a write tool definition
pub fn create_write_tool_definition() -> WriteToolDefinition {
    WriteToolDefinition
}

pub struct WriteToolDefinition;

impl WriteToolDefinition {
    pub fn name(&self) -> &str {
        "write"
    }

    pub fn description(&self) -> &str {
        "Write content to a file. Creates the file if it doesn't exist, overwrites if it does. Automatically creates parent directories."
    }

    pub async fn execute(
        &self,
        path: &str,
        content: &str,
        cwd: &str,
    ) -> Result<WriteOutput, String> {
        let absolute_path = resolve_to_cwd(path, cwd);

        // Create parent directories if needed
        if let Some(parent) = Path::new(&absolute_path).parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create parent directories: {}", e))?;
        }

        // Write the file
        fs::write(&absolute_path, content)
            .await
            .map_err(|e| format!("Failed to write file: {}", e))?;

        Ok(WriteOutput {
            content: vec![serde_json::json!({
                "type": "text",
                "text": format!("Successfully wrote {} bytes to {}", content.len(), path)
            })],
        })
    }
}

pub struct WriteOutput {
    pub content: Vec<serde_json::Value>,
}

// ============================================================================
// Render Functions
// ============================================================================

fn trim_trailing_empty_lines_write(lines: Vec<&str>) -> Vec<&str> {
    let mut count = lines.len();
    while count > 0 && lines[count - 1].trim().is_empty() {
        count -= 1;
    }
    lines[..count].to_vec()
}

/// Render a write tool call — `write /path/to/file` with content preview
pub fn render_write_call(args: &serde_json::Value, ctx: &ToolRenderContext) -> ToolRenderOutput {
    let path = args.get("path").and_then(|v| v.as_str())
        .or_else(|| args.get("file_path").and_then(|v| v.as_str()));
    let content = args.get("content").and_then(|v| v.as_str());

    let path_display = match path {
        Some(p) => ToolTheme::fg("accent", &shorten_path(p)),
        None => invalid_arg_text(&|s| ToolTheme::fg("error", s)),
    };

    let mut label = format!("{} {}",
        ToolTheme::fg("toolTitle", &ToolTheme::bold("write")),
        path_display,
    );

    if let Some(file_content) = content {
        let text = normalize_display_text(file_content);
        let tab_replaced = replace_tabs(&text);
        let lines: Vec<&str> = tab_replaced.split('\n').collect();
        let lines = trim_trailing_empty_lines_write(lines);
        let total_lines = lines.len();
        let max_lines = if ctx.expanded { total_lines } else { 10 };
        let display_lines = &lines[..max_lines.min(total_lines)];
        let remaining = total_lines.saturating_sub(max_lines);

        label.push_str("\n\n");
        label.push_str(
            &display_lines.iter()
                .map(|line| ToolTheme::fg("toolOutput", line))
                .collect::<Vec<_>>()
                .join("\n"),
        );

        if remaining > 0 {
            label.push_str(
                &ToolTheme::fg("muted", &format!("\n... ({} more lines, {} total, use expand to expand)", remaining, total_lines)),
            );
        }
    }

    ToolRenderOutput { label, formatted: String::new() }
}

/// Render a write tool result — error message if failed, empty on success
pub fn render_write_result(
    output: &WriteOutput,
    _options: &ToolRenderOptions,
    ctx: &ToolRenderContext,
) -> ToolRenderOutput {
    if ctx.is_error {
        let error_text: String = output.content
            .iter()
            .filter_map(|c| c.get("text").and_then(|v| v.as_str()))
            .collect::<Vec<_>>()
            .join("\n");
        if !error_text.is_empty() {
            return ToolRenderOutput {
                label: String::new(),
                formatted: format!("\n{}", ToolTheme::fg("error", &error_text)),
            };
        }
    }

    // On success, render nothing extra
    ToolRenderOutput { label: String::new(), formatted: String::new() }
}
