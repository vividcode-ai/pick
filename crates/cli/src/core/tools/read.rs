use super::truncate::*;
use super::path_utils::resolve_to_cwd;
use super::render_utils::{ToolRenderContext, ToolRenderOptions, ToolRenderOutput, ToolTheme,
    shorten_path, invalid_arg_text, get_text_output, normalize_display_text};

/// Create a read tool definition
pub fn create_read_tool_definition() -> ReadToolDefinition {
    ReadToolDefinition
}

pub struct ReadToolDefinition;

impl ReadToolDefinition {
    pub fn name(&self) -> &str {
        "read"
    }

    pub fn description(&self) -> &str {
        "Read the contents of a file. Supports text files and images."
    }

    pub async fn execute(
        &self,
        path: &str,
        cwd: &str,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<ReadOutput, String> {
        let absolute_path = resolve_to_cwd(path, cwd);

        // Check if file exists
        if !tokio::fs::try_exists(&absolute_path).await.map_err(|e| e.to_string())? {
            return Err(format!("File not found: {}", path));
        }

        // Check if it's an image (by extension)
        let is_image = matches!(
            absolute_path.rsplit('.').next().unwrap_or("").to_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "webp"
        );

        if is_image {
            // For images, return the path info (actual image handling depends on the provider)
            return Ok(ReadOutput {
                content: vec![
                    serde_json::json!({"type": "text", "text": format!("Read image file [{}]", get_mime_type(&absolute_path))}),
                ],
                details: None,
            });
        }

        // Read text content
        let content = tokio::fs::read_to_string(&absolute_path)
            .await
            .map_err(|e| format!("Failed to read file: {}", e))?;

        let all_lines: Vec<&str> = content.split('\n').collect();
        let total_file_lines = all_lines.len();
        let start_line = offset.unwrap_or(1).saturating_sub(1);

        if start_line >= all_lines.len() {
            return Err(format!(
                "Offset {} is beyond end of file ({} lines total)",
                offset.unwrap_or(1),
                total_file_lines
            ));
        }

        let selected_content = if let Some(l) = limit {
            let end = (start_line + l).min(all_lines.len());
            all_lines[start_line..end].join("\n")
        } else {
            all_lines[start_line..].join("\n")
        };

        let truncation = truncate_head(&selected_content, TruncationOptions::default());
        let mut output_text = truncation.content.clone();
        let mut details: Option<ReadToolDetails> = None;

        if truncation.first_line_exceeds_limit {
            let start_line_display = start_line + 1;
            let first_line_size = format_size(all_lines[start_line].len());
            output_text = format!(
                "[Line {} is {}, exceeds {} limit. Use bash fallback.]",
                start_line_display, first_line_size, DEFAULT_MAX_BYTES
            );
            details = Some(ReadToolDetails {
                truncation: Some(truncation),
            });
        } else if truncation.truncated {
            let end_line_display = start_line + truncation.output_lines;
            let next_offset = end_line_display + 1;
            if truncation.truncated_by == Some(TruncationType::Lines) {
                output_text.push_str(&format!(
                    "\n\n[Showing lines {}-{} of {}. Use offset={} to continue.]",
                    start_line + 1, end_line_display, total_file_lines, next_offset
                ));
            } else {
                output_text.push_str(&format!(
                    "\n\n[Showing lines {}-{} of {} ({} limit). Use offset={} to continue.]",
                    start_line + 1, end_line_display, total_file_lines, format_size(DEFAULT_MAX_BYTES), next_offset
                ));
            }
            details = Some(ReadToolDetails {
                truncation: Some(truncation),
            });
        }

        Ok(ReadOutput {
            content: vec![serde_json::json!({"type": "text", "text": output_text})],
            details,
        })
    }
}

pub struct ReadOutput {
    pub content: Vec<serde_json::Value>,
    pub details: Option<ReadToolDetails>,
}

pub struct ReadToolDetails {
    pub truncation: Option<TruncationResult>,
}

fn get_mime_type(path: &str) -> &str {
    match path.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

// ============================================================================
// Render Functions
// ============================================================================

fn format_read_line_range(offset: Option<u64>, limit: Option<u64>) -> String {
    match (offset, limit) {
        (Some(o), Some(l)) => ToolTheme::fg("warning", &format!(":{}-{}", o, o + l - 1)),
        (Some(o), None) => ToolTheme::fg("warning", &format!(":{}", o)),
        (None, Some(_)) => ToolTheme::fg("warning", ":1-"),
        (None, None) => String::new(),
    }
}

fn trim_trailing_empty_lines(lines: Vec<&str>) -> Vec<&str> {
    let mut count = lines.len();
    while count > 0 && lines[count - 1].trim().is_empty() {
        count -= 1;
    }
    lines[..count].to_vec()
}

/// Render a read tool call — one-line summary like `read /path/to/file:1-50`
pub fn render_read_call(args: &serde_json::Value, _ctx: &ToolRenderContext) -> ToolRenderOutput {
    let path = args.get("path").and_then(|v| v.as_str())
        .or_else(|| args.get("file_path").and_then(|v| v.as_str()));
    let offset = args.get("offset").and_then(|v| v.as_u64());
    let limit = args.get("limit").and_then(|v| v.as_u64());

    let path_display = match path {
        Some(p) => ToolTheme::fg("accent", &shorten_path(p)),
        None => invalid_arg_text(&|s| ToolTheme::fg("error", s)),
    };

    let range = format_read_line_range(offset, limit);

    let label = ToolTheme::fg("toolTitle", &ToolTheme::bold("read"))
        + " "
        + &path_display
        + &range;

    ToolRenderOutput { label, formatted: String::new() }
}

/// Render a read tool result — file content with line display
pub fn render_read_result(
    output: &ReadOutput,
    options: &ToolRenderOptions,
    ctx: &ToolRenderContext,
) -> ToolRenderOutput {
    if !options.expanded && !ctx.is_error {
        return ToolRenderOutput { label: String::new(), formatted: String::new() };
    }

    let raw = get_text_output(&output.content, ctx.show_images);
    let text = normalize_display_text(&raw);
    let lines: Vec<&str> = text.split('\n').collect();
    let lines = trim_trailing_empty_lines(lines);

    let max_lines = if options.expanded { lines.len() } else { 10 };
    let display_lines = &lines[..max_lines.min(lines.len())];
    let remaining = lines.len().saturating_sub(max_lines);

    let mut formatted = "\n".to_string();
    formatted.push_str(
        &display_lines.iter()
            .map(|line| ToolTheme::fg("toolOutput", line))
            .collect::<Vec<_>>()
            .join("\n"),
    );

    if remaining > 0 {
        formatted.push_str(
            &ToolTheme::fg("muted", &format!("\n... ({} more lines, use expand to expand)", remaining)),
        );
    }

    if let Some(ref details) = output.details {
        if let Some(ref truncation) = details.truncation {
            if truncation.truncated {
                if truncation.first_line_exceeds_limit {
                    formatted.push_str(&format!("\n{}",
                        ToolTheme::fg("warning", &format!("[First line exceeds {} limit]",
                            format_size(truncation.max_bytes)))));
                } else if truncation.truncated_by == Some(TruncationType::Lines) {
                    formatted.push_str(&format!("\n{}",
                        ToolTheme::fg("warning", &format!("[Truncated: showing {} of {} lines ({} line limit)]",
                            truncation.output_lines, truncation.total_lines, truncation.max_lines))));
                } else {
                    formatted.push_str(&format!("\n{}",
                        ToolTheme::fg("warning", &format!("[Truncated: {} lines shown ({} limit)]",
                            truncation.output_lines, format_size(truncation.max_bytes)))));
                }
            }
        }
    }

    ToolRenderOutput { label: String::new(), formatted }
}
