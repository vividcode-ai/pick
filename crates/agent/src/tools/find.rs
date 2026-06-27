//! Find tool - searches for files by glob pattern

use std::path::Path;

use pick_ai::types::ContentBlock;

use crate::core::state::{AgentTool, AgentToolResult, ToolExecutionMode};

/// Convert a glob pattern to a regex string.
/// Handles `**`, `*`, `?`, `.` and other glob metacharacters.
fn glob_to_regex(glob: &str) -> String {
    let mut re = String::with_capacity(glob.len() + 4);
    re.push('^');
    let mut chars = glob.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next();
                    // ** matches everything including path separators
                    re.push_str(".*");
                    if chars.peek() == Some(&'/') {
                        chars.next();
                    }
                } else {
                    // * matches within a single path component
                    re.push_str("[^/]*");
                }
            }
            '?' => re.push_str("[^/]"),
            '.' => re.push_str("\\."),
            '/' => re.push_str("[/\\\\]"),
            '\\' => re.push_str("\\\\"),
            '+' => re.push_str("\\+"),
            '(' => {
                re.push('\\');
                re.push('(');
            }
            ')' => {
                re.push('\\');
                re.push(')');
            }
            '[' => {
                re.push('\\');
                re.push('[');
            }
            ']' => {
                re.push('\\');
                re.push(']');
            }
            '{' => {
                re.push('\\');
                re.push('{');
            }
            '}' => {
                re.push('\\');
                re.push('}');
            }
            '^' => {
                re.push('\\');
                re.push('^');
            }
            '$' => {
                re.push('\\');
                re.push('$');
            }
            '|' => {
                re.push('\\');
                re.push('|');
            }
            '!' => {
                re.push('\\');
                re.push('!');
            }
            c => re.push(c),
        }
    }
    re.push('$');
    re
}

/// Create the find tool definition
pub fn create_find_tool() -> AgentTool {
    let params = pick_ai::types::JsonSchema {
        schema_type: "object".to_string(),
        properties: Some(
            vec![
                (
                    "pattern".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Glob pattern to match files (required), e.g. '*.rs', '**/*.json', or 'src/**/*.spec.rs'"
                    }),
                ),
                (
                    "path".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Directory to search in (default: current directory)"
                    }),
                ),
                (
                    "limit".to_string(),
                    serde_json::json!({
                        "type": "number",
                        "description": "Maximum number of results (default: 1000)"
                    }),
                ),
            ]
            .into_iter()
            .collect(),
        ),
        required: Some(vec!["pattern".to_string()]),
        description: Some("Search for files by glob pattern. Returns matching file paths relative to the search directory. Skips hidden files and directories. Example: find(pattern: \"*.rs\", path: \"src/\")".to_string()),
        items: None,
        additional_properties: Some(false),
    };

    AgentTool {
        name: "find".to_string(),
        description: "Search for files by glob pattern. Returns matching file paths relative to the search directory. Skips hidden files and directories. Example: find(pattern: \"*.rs\", path: \"src/\")".to_string(),
        prompt_snippet: Some("Find files by glob pattern".to_string()),
        prompt_guidelines: vec![],
        label: "find".to_string(),
        parameters: params,
        execute: std::sync::Arc::new(|_tool_call_id, args, ctx| {
            Box::pin(async move {
                let pattern = args.get("pattern").and_then(|v| v.as_str()).ok_or_else(|| "Missing pattern".to_string())?;
                let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let path = Path::new(path_str);
                let cwd = ctx.cwd.as_deref().unwrap_or_else(|| Path::new("."));
                // Check fs_policy on the search root path
                if let Some(ref policy) = ctx.fs_policy {
                    if let Err(e) = policy.can_read(path, cwd) {
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
                                    "Find",
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
                }
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(1000) as usize;

                let regex_str = glob_to_regex(pattern);
                let re = regex::Regex::new(&regex_str)
                    .map_err(|e| format!("Invalid glob pattern '{}': {}", pattern, e))?;

                let mut results: Vec<String> = Vec::new();

                let walker = walkdir::WalkDir::new(path)
                    .follow_links(false)
                    .into_iter()
                    .filter_entry(|e| {
                        !e.file_name().to_string_lossy().starts_with('.')
                    });

                for entry in walker.filter_map(|e| e.ok()) {
                    if results.len() >= limit {
                        break;
                    }
                    if !entry.file_type().is_file() {
                        continue;
                    }
                    let full_path = entry.path();
                    let relative = full_path.strip_prefix(path).unwrap_or(full_path);
                    let name = relative.to_string_lossy();
                    if re.is_match(&name) {
                        results.push(format!("./{}", name));
                    }
                }

                let output = if results.is_empty() {
                    "No files found".to_string()
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
        execution_mode: ToolExecutionMode::Sequential,
    }
}
