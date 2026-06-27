//! Ls tool - lists directory contents

use std::path::Path;

use pick_ai::types::ContentBlock;

use crate::core::state::{AgentTool, AgentToolResult, ToolExecutionMode};

/// Create the ls tool definition
pub fn create_ls_tool() -> AgentTool {
    let params = pick_ai::types::JsonSchema {
        schema_type: "object".to_string(),
        properties: Some(
            vec![
                (
                    "path".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Directory to list (default: current directory)"
                    }),
                ),
                (
                    "limit".to_string(),
                    serde_json::json!({
                        "type": "number",
                        "description": "Maximum number of entries to return (default: 500)"
                    }),
                ),
            ]
            .into_iter()
            .collect(),
        ),
        required: Some(vec![]),
        description: Some("List directory contents. Returns entries sorted alphabetically, with '/' suffix for directories. Includes dotfiles. Example: ls(path: \"src/\")".to_string()),
        items: None,
        additional_properties: Some(false),
    };

    AgentTool {
        name: "ls".to_string(),
        description: "List directory contents. Returns entries sorted alphabetically, with '/' suffix for directories. Includes dotfiles. Example: ls(path: \"src/\")".to_string(),
        prompt_snippet: Some("List directory contents".to_string()),
        prompt_guidelines: vec![],
        label: "ls".to_string(),
        parameters: params,
        execute: std::sync::Arc::new(|_tool_call_id, args, ctx| {
            Box::pin(async move {
                let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let limit = args.get("limit").and_then(|v| v.as_u64());

                let path = Path::new(path_str);
                let cwd = ctx.cwd.as_deref().unwrap_or_else(|| Path::new("."));

                // Check fs_policy
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
                                    "Ls",
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

                let mut entries = tokio::fs::read_dir(path).await
                    .map_err(|e| format!("Failed to read directory: {}", e))?;

                let mut result = String::new();
                let mut count = 0u64;
                while let Some(entry) = entries.next_entry().await
                    .map_err(|e| format!("Error reading entry: {}", e))?
                {
                    if let Some(lim) = limit
                        && count >= lim {
                            result.push_str(&format!("... (limit of {} entries reached)", lim));
                            break;
                        }

                    let file_name = entry.file_name().to_string_lossy().to_string();
                    let file_type = entry.file_type().await
                        .map_err(|e| format!("Error getting file type: {}", e))?;

                    if file_type.is_dir() {
                        result.push_str(&format!("{}/\n", file_name));
                    } else if file_type.is_symlink() {
                        result.push_str(&format!("{}@\n", file_name));
                    } else {
                        result.push_str(&format!("{}\n", file_name));
                    }
                    count += 1;
                }

                if result.is_empty() {
                    result = "(empty directory)".to_string();
                }

                Ok(AgentToolResult {
                    content: vec![ContentBlock::text(result)],
                    is_error: false,
                    terminate: false,
                })
            })
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}
