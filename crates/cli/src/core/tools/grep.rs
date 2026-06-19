use tokio::process::Command;

use super::path_utils::resolve_to_cwd;
use super::render_utils::{
    ToolRenderContext, ToolRenderOptions, ToolRenderOutput, ToolTheme, shorten_path,
};
use super::truncate::*;

/// Create a grep tool definition
pub fn create_grep_tool_definition() -> GrepToolDefinition {
    GrepToolDefinition
}

pub struct GrepToolDefinition;

impl GrepToolDefinition {
    pub fn name(&self) -> &str {
        "grep"
    }

    pub fn description(&self) -> &str {
        "Search file contents for a pattern using ripgrep."
    }

    pub async fn execute(
        &self,
        pattern: &str,
        cwd: &str,
        search_path: Option<&str>,
        glob: Option<&str>,
        ignore_case: bool,
        literal: bool,
        context: Option<usize>,
        limit: Option<usize>,
    ) -> Result<GrepOutput, String> {
        let search_path = resolve_to_cwd(search_path.unwrap_or("."), cwd);
        let effective_limit = limit.unwrap_or(100).max(1);
        let context_value = context.unwrap_or(0);

        // Check if ripgrep is available
        let rg_check = Command::new("rg").arg("--version").output().await;
        if rg_check.is_err() {
            return Err("ripgrep (rg) is not available. Please install ripgrep first.".to_string());
        }

        let mut args: Vec<String> = vec![
            "--json".into(),
            "--line-number".into(),
            "--color=never".into(),
            "--hidden".into(),
        ];
        if ignore_case {
            args.push("--ignore-case".into());
        }
        if literal {
            args.push("--fixed-strings".into());
        }
        if let Some(g) = glob {
            args.push("--glob".into());
            args.push(g.into());
        }
        args.push("--".into());
        args.push(pattern.into());
        args.push(search_path.clone());

        let output = Command::new("rg")
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await
            .map_err(|e| format!("Failed to run ripgrep: {}", e))?;

        if !output.status.success() && output.status.code() != Some(1) {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.trim().is_empty() {
                return Err(format!("ripgrep error: {}", stderr.trim()));
            }
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            return Ok(GrepOutput {
                content: "No matches found".to_string(),
                details: None,
            });
        }

        // Parse JSON lines from ripgrep
        let mut match_lines: Vec<String> = Vec::new();
        let mut match_count = 0;
        let mut lines_truncated = false;

        for line in stdout.lines() {
            if match_count >= effective_limit {
                break;
            }
            if line.trim().is_empty() {
                continue;
            }

            if let Ok(event) = serde_json::from_str::<serde_json::Value>(line)
                && event["type"] == "match"
                && let (Some(file_path), Some(line_number)) = (
                    event["data"]["path"]["text"].as_str(),
                    event["data"]["line_number"].as_u64(),
                )
            {
                let rel_path = if file_path.starts_with(&search_path) {
                    &file_path[search_path.len() + 1..]
                } else {
                    file_path
                };
                let line_text = event["data"]["lines"]["text"]
                    .as_str()
                    .unwrap_or("")
                    .trim_end_matches('\n');

                let (truncated_text, was_truncated) =
                    truncate_line(line_text, GREP_MAX_LINE_LENGTH);
                if was_truncated {
                    lines_truncated = true;
                }

                if context_value > 0 {
                    match_lines.push(format!("{}:{}: {}", rel_path, line_number, truncated_text));
                } else {
                    match_lines.push(format!("{}:{}: {}", rel_path, line_number, truncated_text));
                }
                match_count += 1;
            }
        }

        if match_lines.is_empty() {
            return Ok(GrepOutput {
                content: "No matches found".to_string(),
                details: None,
            });
        }

        let raw_output = match_lines.join("\n");
        let truncation = truncate_head(&raw_output, TruncationOptions::default());
        let truncation_truncated = truncation.truncated;
        let mut output_text = truncation.content.clone();
        let match_limit_reached = match_count >= effective_limit;

        let details = GrepToolDetails {
            truncation: if truncation_truncated {
                Some(truncation)
            } else {
                None
            },
            match_limit_reached: if match_limit_reached {
                Some(effective_limit)
            } else {
                None
            },
            lines_truncated: if lines_truncated { Some(true) } else { None },
        };

        let mut notices: Vec<String> = Vec::new();
        if match_limit_reached {
            notices.push(format!(
                "{} matches limit reached. Use limit={} for more, or refine pattern",
                effective_limit,
                effective_limit * 2
            ));
        }
        if truncation_truncated {
            notices.push(format!("{} limit reached", format_size(DEFAULT_MAX_BYTES)));
        }
        if lines_truncated {
            notices.push(format!(
                "Some lines truncated to {} chars. Use read tool to see full lines",
                GREP_MAX_LINE_LENGTH
            ));
        }
        if !notices.is_empty() {
            output_text.push_str(&format!("\n\n[{}]", notices.join(". ")));
        }

        Ok(GrepOutput {
            content: output_text,
            details: Some(details),
        })
    }
}

pub struct GrepOutput {
    pub content: String,
    pub details: Option<GrepToolDetails>,
}

pub struct GrepToolDetails {
    pub truncation: Option<TruncationResult>,
    pub match_limit_reached: Option<usize>,
    pub lines_truncated: Option<bool>,
}

// ============================================================================
// Render Functions
// ============================================================================

/// Render a grep tool call — `grep /pattern/ in /path (glob) limit N`
pub fn render_grep_call(args: &serde_json::Value, _ctx: &ToolRenderContext) -> ToolRenderOutput {
    let pattern = args.get("pattern").and_then(|v| v.as_str());
    let raw_path = args.get("path").and_then(|v| v.as_str());
    let glob = args.get("glob").and_then(|v| v.as_str());
    let limit = args.get("limit").and_then(|v| v.as_u64());

    let path_display = match raw_path {
        Some(p) if !p.is_empty() => shorten_path(p),
        _ => ".".to_string(),
    };

    let mut label = format!(
        "{} /{}/ in {}",
        ToolTheme::fg("toolTitle", &ToolTheme::bold("grep")),
        ToolTheme::fg("accent", pattern.unwrap_or("")),
        ToolTheme::fg("toolOutput", &path_display),
    );

    if let Some(g) = glob {
        label.push_str(&ToolTheme::fg("toolOutput", &format!(" ({})", g)));
    }
    if let Some(l) = limit {
        label.push_str(&ToolTheme::fg("toolOutput", &format!(" limit {}", l)));
    }

    ToolRenderOutput {
        label,
        formatted: String::new(),
    }
}

/// Render a grep tool result — match lines with warnings
pub fn render_grep_result(
    output: &GrepOutput,
    options: &ToolRenderOptions,
    _ctx: &ToolRenderContext,
) -> ToolRenderOutput {
    let mut formatted = String::new();
    let content = output.content.trim();

    if !content.is_empty() {
        let lines: Vec<&str> = content.split('\n').collect();
        let max_lines = if options.expanded { lines.len() } else { 15 };
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
        if let Some(ml) = details.match_limit_reached {
            warnings.push(format!("{} matches limit", ml));
        }
        if let Some(ref truncation) = details.truncation
            && truncation.truncated
        {
            warnings.push(format!("{} limit", format_size(truncation.max_bytes)));
        }
        if details.lines_truncated.unwrap_or(false) {
            warnings.push("some lines truncated".to_string());
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
