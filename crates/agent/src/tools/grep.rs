//! Grep tool - searches file contents

use std::fs;
use std::path::Path;

use pick_ai::types::ContentBlock;

use crate::core::state::{AgentTool, AgentToolResult, ToolExecutionMode};

/// Convert a simple glob pattern to a regex string for file filtering.
fn glob_to_regex(glob: &str) -> String {
    let mut re = String::with_capacity(glob.len() + 4);
    re.push('^');
    let mut chars = glob.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next();
                    re.push_str(".*");
                    if chars.peek() == Some(&'/') {
                        chars.next();
                    }
                } else {
                    re.push_str("[^/]*");
                }
            }
            '?' => re.push_str("[^/]"),
            '.' => re.push_str("\\."),
            '/' => re.push_str("[/\\\\]"),
            '\\' => re.push_str("\\\\"),
            '+' => re.push_str("\\+"),
            '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '!' => {
                re.push('\\');
                re.push(ch);
            }
            c => re.push(c),
        }
    }
    re.push('$');
    re
}

/// Create the grep tool definition
pub fn create_grep_tool() -> AgentTool {
    let params = pick_ai::types::JsonSchema {
        schema_type: "object".to_string(),
        properties: Some(
            vec![
                (
                    "pattern".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Search pattern (regex or literal string, required)"
                    }),
                ),
                (
                    "path".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Directory or file to search (default: current directory)"
                    }),
                ),
                (
                    "glob".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Filter files by glob pattern, e.g. '*.rs' or '**/*.spec.rs'"
                    }),
                ),
                (
                    "ignoreCase".to_string(),
                    serde_json::json!({
                        "type": "boolean",
                        "description": "Case-insensitive search (default: false)"
                    }),
                ),
                (
                    "literal".to_string(),
                    serde_json::json!({
                        "type": "boolean",
                        "description": "Treat pattern as literal string instead of regex (default: false)"
                    }),
                ),
                (
                    "context".to_string(),
                    serde_json::json!({
                        "type": "number",
                        "description": "Number of lines to show before and after each match (default: 0)"
                    }),
                ),
                (
                    "limit".to_string(),
                    serde_json::json!({
                        "type": "number",
                        "description": "Maximum number of matches to return (default: 100)"
                    }),
                ),
            ]
            .into_iter()
            .collect(),
        ),
        required: Some(vec!["pattern".to_string()]),
        description: Some("Search file contents for a pattern. Returns matching lines with file paths and line numbers. Skips hidden files and directories. Example: grep(pattern: \"TODO\", path: \"src/\")".to_string()),
        items: None,
        additional_properties: Some(false),
    };

    AgentTool {
        name: "grep".to_string(),
        description: "Search file contents for a pattern. Returns matching lines with file paths and line numbers. Skips hidden files and directories. Example: grep(pattern: \"TODO\", path: \"src/\")".to_string(),
        prompt_snippet: Some("Search file contents for patterns".to_string()),
        prompt_guidelines: vec![],
        usage_example: Some(vec!["grep(pattern: \"TODO\", path: \"src/\")".to_string()]),
        label: "grep".to_string(),
        parameters: params,
        execute: std::sync::Arc::new(|_tool_call_id, args, ctx| {
            Box::pin(async move {
                let pattern_str = args.get("pattern").and_then(|v| v.as_str()).ok_or_else(|| "Missing pattern".to_string())?;
                let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let path = Path::new(path_str);
                let cwd = ctx.cwd.as_deref().unwrap_or_else(|| Path::new("."));
                // Check fs_policy on the search root path
                if let Some(ref policy) = ctx.fs_policy
                    && let Err(e) = policy.can_read(path, cwd)
                {
                        // Protected paths (e.g. .git/**) are hard denied
                        if policy.is_path_protected(path, cwd).unwrap_or(false) {
                            return Ok(AgentToolResult {
                                content: vec![ContentBlock::text(format!("FsPolicy: {}", e))],
                                is_error: true,
                                terminate: false,
                            });
                        }
                        // External paths: check authorization
                        if let Some(ref pm) = ctx.permission_manager {
                            let authorized =
                                crate::permission::external_dir::check_authorization(
                                    "Grep",
                                    path_str,
                                    pm,
                                    ctx.question.as_ref(),
                                    ctx.tool_event_bus.as_ref(),
                                )
                                .await?;
                            if !authorized {
                                return Ok(AgentToolResult {
                                    content: vec![ContentBlock::text(format!(
                                        "Error: Access denied: '{}' is outside the allowed workspace",
                                        path_str
                                    ))],
                                    is_error: true,
                                    terminate: false,
                                });
                            }
                        } else {
                            return Ok(AgentToolResult {
                                content: vec![ContentBlock::text(format!("FsPolicy: {}", e))],
                                is_error: true,
                                terminate: false,
                            });
                        }
                }
                let glob_filter = args.get("glob").and_then(|v| v.as_str());
                let ignore_case = args.get("ignoreCase").and_then(|v| v.as_bool()).unwrap_or(false);
                let literal = args.get("literal").and_then(|v| v.as_bool()).unwrap_or(false);
                let context_lines = args.get("context").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

                // Build regex pattern
                let raw = if literal {
                    regex::escape(pattern_str)
                } else {
                    pattern_str.to_string()
                };
                let re = if ignore_case {
                    regex::RegexBuilder::new(&raw)
                        .case_insensitive(true)
                        .build()
                        .map_err(|e| format!("Invalid pattern: {}", e))?
                } else {
                    regex::Regex::new(&raw)
                        .map_err(|e| format!("Invalid pattern: {}", e))?
                };

                // Build glob filter regex if specified
                let glob_re = glob_filter
                    .and_then(|g| regex::Regex::new(&glob_to_regex(g)).ok());

                let mut results: Vec<String> = Vec::new();
                let mut count = 0;

                let walker = walkdir::WalkDir::new(path)
                    .follow_links(false)
                    .into_iter()
                    .filter_entry(|e| {
                        !e.file_name().to_string_lossy().starts_with('.')
                    });

                for entry in walker.filter_map(|e| e.ok()) {
                    if count >= limit {
                        break;
                    }
                    if !entry.file_type().is_file() {
                        continue;
                    }

                    // Apply glob filter
                    if let Some(ref gre) = glob_re {
                        let rel = entry.path().strip_prefix(path).unwrap_or(entry.path());
                        if !gre.is_match(&rel.to_string_lossy()) {
                            continue;
                        }
                    }

                    let content = match fs::read_to_string(entry.path()) {
                        Ok(c) => c,
                        Err(_) => continue,
                    };

                    let file_path = entry.path().display().to_string();
                    let lines: Vec<&str> = content.lines().collect();

                    for (linenum, line) in lines.iter().enumerate() {
                        if count >= limit {
                            break;
                        }
                        if re.is_match(line) {
                            if context_lines > 0 {
                                let start = linenum.saturating_sub(context_lines);
                                let end = std::cmp::min(linenum + context_lines + 1, lines.len());
                                if !results.is_empty() {
                                    results.push("--".to_string());
                                }
                                for ctx_linenum in start..end {
                                    let sep = if ctx_linenum == linenum { ":" } else { "-" };
                                    results.push(format!("{}:{}{}{}", file_path, ctx_linenum + 1, sep, lines[ctx_linenum]));
                                }
                            } else {
                                results.push(format!("{}:{}:{}", file_path, linenum + 1, line));
                            }
                            count += 1;
                        }
                    }
                }

                let output = if results.is_empty() {
                    "No matches found".to_string()
                } else {
                    results.join("\n")
                };

                Ok(AgentToolResult {
                    content: vec![ContentBlock::text(output)],
                    is_error: false,
                    terminate: false,
                })
            })
        }),
        execution_mode: ToolExecutionMode::Parallel,
    }
}
