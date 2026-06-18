use tokio::process::Command;

use super::path_utils::resolve_to_cwd;
use super::render_utils::{
    ToolRenderContext, ToolRenderOptions, ToolRenderOutput, ToolTheme, shorten_path,
};
use super::truncate::*;

/// Create a find tool definition
pub fn create_find_tool_definition() -> FindToolDefinition {
    FindToolDefinition
}

pub struct FindToolDefinition;

impl FindToolDefinition {
    pub fn name(&self) -> &str {
        "find"
    }

    pub fn description(&self) -> &str {
        "Search for files by glob pattern using fd."
    }

    pub async fn execute(
        &self,
        pattern: &str,
        cwd: &str,
        search_path: Option<&str>,
        limit: Option<usize>,
    ) -> Result<FindOutput, String> {
        let search_path = resolve_to_cwd(search_path.unwrap_or("."), cwd);
        let effective_limit = limit.unwrap_or(1000);

        // Check if fd is available
        let fd_check = Command::new("fd").arg("--version").output().await;
        if fd_check.is_err() {
            return Err("fd is not available. Please install fd first.".to_string());
        }

        let mut args: Vec<String> = vec![
            "--glob".into(),
            "--color=never".into(),
            "--hidden".into(),
            "--no-require-git".into(),
            "--max-results".into(),
            effective_limit.to_string(),
        ];

        let mut effective_pattern = pattern.to_string();
        if pattern.contains('/') {
            args.push("--full-path".into());
            if !pattern.starts_with('/') && !pattern.starts_with("**/") && pattern != "**" {
                effective_pattern = format!("**/{}", pattern);
            }
        }
        args.push("--".into());
        args.push(effective_pattern);
        args.push(search_path.clone());

        let output = Command::new("fd")
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await
            .map_err(|e| format!("Failed to run fd: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.trim().is_empty() {
                return Err(format!("fd error: {}", stderr.trim()));
            }
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            return Ok(FindOutput {
                content: "No files found matching pattern".to_string(),
                details: None,
            });
        }

        // Process results: relativize paths
        let lines: Vec<&str> = stdout.lines().collect();
        let relativized: Vec<String> = lines
            .iter()
            .map(|line| {
                let line = line.trim_end_matches('\r');
                if line.starts_with(&search_path) {
                    line[search_path.len() + 1..].to_string()
                } else {
                    line.to_string()
                }
            })
            .collect();

        let result_limit_reached = relativized.len() >= effective_limit;
        let raw_output = relativized.join("\n");
        let truncation = truncate_head(&raw_output, TruncationOptions::default());
        let truncation_truncated = truncation.truncated;
        let mut output_text = truncation.content.clone();

        let details = FindToolDetails {
            truncation: if truncation_truncated {
                Some(truncation)
            } else {
                None
            },
            result_limit_reached: if result_limit_reached {
                Some(effective_limit)
            } else {
                None
            },
        };

        let mut notices: Vec<String> = Vec::new();
        if result_limit_reached {
            notices.push(format!(
                "{} results limit reached. Use limit={} for more, or refine pattern",
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

        Ok(FindOutput {
            content: output_text,
            details: Some(details),
        })
    }
}

pub struct FindOutput {
    pub content: String,
    pub details: Option<FindToolDetails>,
}

pub struct FindToolDetails {
    pub truncation: Option<TruncationResult>,
    pub result_limit_reached: Option<usize>,
}

// ============================================================================
// Render Functions
// ============================================================================

/// Render a find tool call — `find pattern in /path (limit N)`
pub fn render_find_call(args: &serde_json::Value, _ctx: &ToolRenderContext) -> ToolRenderOutput {
    let pattern = args.get("pattern").and_then(|v| v.as_str());
    let raw_path = args.get("path").and_then(|v| v.as_str());
    let limit = args.get("limit").and_then(|v| v.as_u64());

    let path_display = match raw_path {
        Some(p) if !p.is_empty() => shorten_path(p),
        _ => ".".to_string(),
    };

    let mut label = format!(
        "{} {} in {}",
        ToolTheme::fg("toolTitle", &ToolTheme::bold("find")),
        ToolTheme::fg("accent", pattern.unwrap_or("")),
        ToolTheme::fg("toolOutput", &path_display),
    );

    if let Some(l) = limit {
        label.push_str(&ToolTheme::fg("toolOutput", &format!(" (limit {})", l)));
    }

    ToolRenderOutput {
        label,
        formatted: String::new(),
    }
}

/// Render a find tool result — file paths with warnings
pub fn render_find_result(
    output: &FindOutput,
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
            &display_lines
                .iter()
                .map(|line| ToolTheme::fg("toolOutput", line))
                .collect::<Vec<_>>()
                .join("\n"),
        );

        if remaining > 0 {
            formatted.push_str(&ToolTheme::fg(
                "muted",
                &format!("\n... ({} more lines, use expand to expand)", remaining),
            ));
        }
    }

    if let Some(ref details) = output.details {
        let mut warnings = Vec::new();
        if let Some(rl) = details.result_limit_reached {
            warnings.push(format!("{} results limit", rl));
        }
        if let Some(ref truncation) = details.truncation {
            if truncation.truncated {
                warnings.push(format!("{} limit", format_size(truncation.max_bytes)));
            }
        }
        if !warnings.is_empty() {
            formatted.push_str(&format!(
                "\n{}",
                ToolTheme::fg("warning", &format!("[Truncated: {}]", warnings.join(", ")))
            ));
        }
    }

    ToolRenderOutput {
        label: String::new(),
        formatted,
    }
}
