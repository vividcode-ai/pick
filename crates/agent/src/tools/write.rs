//! Write tool - writes content to files

use pick_ai::types::ContentBlock;

use crate::core::state::{AgentTool, AgentToolResult, ToolExecutionMode};

/// Create the write tool definition
pub fn create_write_tool() -> AgentTool {
    let params = pick_ai::types::JsonSchema {
        schema_type: "object".to_string(),
        properties: Some(
            vec![
                (
                    "path".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Path to the file to write (relative or absolute)"
                    }),
                ),
                (
                    "file_path".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Alternative name for 'path'"
                    }),
                ),
                (
                    "content".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Content to write to the file"
                    }),
                ),
            ]
            .into_iter()
            .collect(),
        ),
        required: Some(vec!["path".to_string(), "content".to_string()]),
        description: Some("Write content to a file. Creates the file if it doesn't exist, overwrites if it does. Automatically creates parent directories.".to_string()),
        items: None,
        additional_properties: Some(false),
    };

    AgentTool {
        name: "write".to_string(),
        description: "Write content to a file. Creates the file if it doesn't exist, overwrites if it does. Automatically creates parent directories.".to_string(),
        prompt_snippet: Some("Create or overwrite files".to_string()),
        prompt_guidelines: vec!["Use write only for new files or complete rewrites.".to_string()],
        label: "write".to_string(),
        parameters: params,
        execute: std::sync::Arc::new(|_tool_call_id, args, ctx| {
            Box::pin(async move {
                let file_path = args
                    .get("file_path")
                    .or_else(|| args.get("path"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing path argument".to_string())?;
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing content argument".to_string())?;

                if let (Some(ref policy), Some(ref cwd)) = (ctx.fs_policy, ctx.cwd) {
                    if let Err(_e) = policy.can_write(std::path::Path::new(file_path), cwd) {
                        // Protected paths (e.g. .git/**) are hard denied, not authorizable
                        if policy.is_path_protected(std::path::Path::new(file_path), cwd).unwrap_or(false) {
                            return Ok(AgentToolResult {
                                content: vec![ContentBlock::text(format!("FsPolicy: {}", _e))],
                                is_error: true, terminate: false,
                            });
                        }
                        // External paths: check authorization
                        if let Some(ref pm) = ctx.permission_manager {
                            let authorized = crate::permission::external_dir::check_authorization(
                                "Write", file_path, pm, ctx.question.as_ref(),
                            ).await?;
                            if !authorized {
                                return Ok(AgentToolResult {
                                    content: vec![ContentBlock::text(format!(
                                        "Error: Write access denied: '{}' is outside the allowed workspace", file_path
                                    ))],
                                    is_error: true, terminate: false,
                                });
                            }
                        } else {
                            return Ok(AgentToolResult {
                                content: vec![ContentBlock::text(format!("FsPolicy: {}", _e))],
                                is_error: true, terminate: false,
                            });
                        }
                    }
                }

                // Ensure parent directory exists
                if let Some(parent) = std::path::Path::new(file_path).parent() {
                    tokio::fs::create_dir_all(parent).await.map_err(|e| format!("Failed to create directory: {}", e))?;
                }

                match tokio::fs::write(file_path, content).await {
                    Ok(_) => Ok(AgentToolResult {
                        content: vec![ContentBlock::text(format!("Successfully wrote {} bytes to {}", content.len(), file_path))],
                        is_error: false,
                        terminate: false,
                    }),
                    Err(e) => Ok(AgentToolResult {
                        content: vec![ContentBlock::text(format!("Error writing file: {}", e))],
                        is_error: true,
                        terminate: false,
                    }),
                }
            })
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}
