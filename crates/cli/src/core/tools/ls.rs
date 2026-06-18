use tokio::fs;

use super::truncate::*;
use super::path_utils::resolve_to_cwd;
use super::render_utils::{ToolRenderContext, ToolRenderOptions, ToolRenderOutput, ToolTheme,
    shorten_path};

/// Create an ls tool definition
pub fn create_ls_tool_definition() -> LsToolDefinition {
    LsToolDefinition
}

pub struct LsToolDefinition;

impl LsToolDefinition {
    pub fn name(&self) -> &str {
        "ls"
    }

    pub fn description(&self) -> &str {
        "List directory contents."
    }

    pub async fn execute(
        &self,
        cwd: &str,
        path: Option<&str>,
        limit: Option<usize>,
    ) -> Result<LsOutput, String> {
        let dir_path = resolve_to_cwd(path.unwrap_or("."), cwd);
        let effective_limit = limit.unwrap_or(500);

        // Check if path exists
        if !tokio::fs::try_exists(&dir_path).await.map_err(|e| e.to_string())? {
            return Err(format!("Path not found: {}", dir_path));
        }

        // Check if it's a directory
        let metadata = fs::metadata(&dir_path)
            .await
            .map_err(|e| format!("Cannot access path: {}", e))?;
        if !metadata.is_dir() {
            return Err(format!("Not a directory: {}", dir_path));
        }

        // Read directory entries
        let mut entries = fs::read_dir(&dir_path)
            .await
            .map_err(|e| format!("Cannot read directory: {}", e))?;

        let mut results: Vec<String> = Vec::new();
        let mut entry_limit_reached = false;

        while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
            if results.len() >= effective_limit {
                entry_limit_reached = true;
                break;
            }
            let file_name = entry.file_name().to_string_lossy().to_string();
            let suffix = if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                "/"
            } else {
                ""
            };
            results.push(format!("{}{}", file_name, suffix));
        }

        // Sort alphabetically, case-insensitive
        results.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

        if results.is_empty() {
            return Ok(LsOutput {
                content: "(empty directory)".to_string(),
                details: None,
            });
        }

        let raw_output = results.join("\n");
        let truncation = truncate_head(&raw_output, TruncationOptions::default());
        let truncation_truncated = truncation.truncated;
        let mut output_text = truncation.content.clone();

        let details = LsToolDetails {
            truncation: if truncation_truncated { Some(truncation) } else { None },
            entry_limit_reached: if entry_limit_reached { Some(effective_limit) } else { None },
        };

        let mut notices: Vec<String> = Vec::new();
        if entry_limit_reached {
            notices.push(format!(
                "{} entries limit reached. Use limit={} for more",
                effective_limit,
                effective_limit * 2
            ));
        }
        if truncation_truncated {
            notices.push(format!("{} limit reached", format_size(DEFAULT_MAX_BYTES)));
        }
        if !notices.is_empty() {
            output_text.push_str(&format!("\n\n[{}]", notices.join(". ")));
        }

        Ok(LsOutput {
            content: output_text,
            details: Some(details),
        })
    }
}

pub struct LsOutput {
    pub content: String,
    pub details: Option<LsToolDetails>,
}

pub struct LsToolDetails {
    pub truncation: Option<TruncationResult>,
    pub entry_limit_reached: Option<usize>,
}

// ============================================================================
// Render Functions
// ============================================================================

/// Render an ls tool call — `ls /path (limit N)`
pub fn render_ls_call(args: &serde_json::Value, _ctx: &ToolRenderContext) -> ToolRenderOutput {
    let raw_path = args.get("path").and_then(|v| v.as_str());
    let limit = args.get("limit").and_then(|v| v.as_u64());

    let path_display = match raw_path {
        Some(p) if !p.is_empty() => ToolTheme::fg("accent", &shorten_path(p)),
        _ => ToolTheme::fg("accent", "."),
    };

    let mut label = format!("{} {}",
        ToolTheme::fg("toolTitle", &ToolTheme::bold("ls")),
        path_display,
    );

    if let Some(l) = limit {
        label.push_str(&ToolTheme::fg("toolOutput", &format!(" (limit {})", l)));
    }

    ToolRenderOutput { label, formatted: String::new() }
}

/// Render an ls tool result — directory entries with warnings
pub fn render_ls_result(
    output: &LsOutput,
    options: &ToolRenderOptions,
    _ctx: &ToolRenderContext,
) -> ToolRenderOutput {
    let mut formatted = String::new();
    let content = output.content.trim();

    if !content.is_empty() {
        let lines: Vec<&str> = content.split('\n').collect();
        let max_lines = if options.expanded { lines.len() } else { 20 };
        let display_lines = &lines[..max_lines.min(lines.len())];
        let remaining = lines.len().saturating_sub(max_lines);

        formatted.push('\n');
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
    }

    if let Some(ref details) = output.details {
        let mut warnings = Vec::new();
        if let Some(el) = details.entry_limit_reached {
            warnings.push(format!("{} entries limit", el));
        }
        if let Some(ref truncation) = details.truncation {
            if truncation.truncated {
                warnings.push(format!("{} limit", format_size(truncation.max_bytes)));
            }
        }
        if !warnings.is_empty() {
            formatted.push_str(&format!("\n{}",
                ToolTheme::fg("warning", &format!("[Truncated: {}]", warnings.join(", ")))));
        }
    }

    ToolRenderOutput { label: String::new(), formatted }
}
