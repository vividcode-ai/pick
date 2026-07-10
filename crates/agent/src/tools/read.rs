//! Read tool - reads file contents (text) or image data

use pick_ai::types::ContentBlock;
use tokio::io::AsyncReadExt;

use crate::core::state::{AgentTool, AgentToolResult, ToolExecutionMode};

/// Detect image MIME type by sniffing magic bytes from the file header
fn detect_image_mime_type(buf: &[u8]) -> Option<&'static str> {
    if buf.len() < 4 {
        return None;
    }
    // JPEG: starts with FF D8 FF
    if buf[0] == 0xFF && buf[1] == 0xD8 && buf[2] == 0xFF {
        // Exclude JPEG 2000 (FF D8 FF F7 is a JP2 signature)
        if buf.len() > 3 && buf[3] == 0xF7 {
            return None;
        }
        return Some("image/jpeg");
    }
    // PNG: 89 50 4E 47 0D 0A 1A 0A
    if buf.len() >= 8
        && buf[0] == 0x89
        && buf[1] == b'P'
        && buf[2] == b'N'
        && buf[3] == b'G'
        && buf[4] == 0x0D
        && buf[5] == 0x0A
        && buf[6] == 0x1A
        && buf[7] == 0x0A
    {
        return Some("image/png");
    }
    // GIF: "GIF87a" or "GIF89a"
    if buf.len() >= 6
        && buf[0] == b'G'
        && buf[1] == b'I'
        && buf[2] == b'F'
        && (buf[3] == b'8' || buf[3] == b'9')
        && buf[4] == b'a'
    {
        return Some("image/gif");
    }
    // WebP: RIFF .... WEBP
    if buf.len() >= 12
        && buf[0] == b'R'
        && buf[1] == b'I'
        && buf[2] == b'F'
        && buf[3] == b'F'
        && buf[8] == b'W'
        && buf[9] == b'E'
        && buf[10] == b'B'
        && buf[11] == b'P'
    {
        return Some("image/webp");
    }
    None
}

/// Read a file, returning text content or image data depending on MIME type
pub async fn read_file(file_path: &str) -> Result<Vec<ContentBlock>, String> {
    // Try to detect MIME type first by reading the file header
    let mut file = tokio::fs::File::open(file_path)
        .await
        .map_err(|e| format!("Error opening file: {}", e))?;

    let mut header = vec![0u8; 16];
    let n = file
        .read(&mut header)
        .await
        .map_err(|e| format!("Error reading file header: {}", e))?;
    header.truncate(n);

    if n == 0 {
        return Ok(vec![ContentBlock::text("(empty file)")]);
    }

    if let Some(mime_type) = detect_image_mime_type(&header) {
        // Image file: read the rest as binary, base64 encode
        let mut data = header;
        let mut rest = Vec::new();
        file.read_to_end(&mut rest)
            .await
            .map_err(|e| format!("Error reading image file: {}", e))?;
        data.extend(rest);

        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&data);

        Ok(vec![
            ContentBlock::text(format!("Read image file [{}]", mime_type)),
            ContentBlock::image(b64, mime_type),
        ])
    } else {
        // Text file: read as string (drop the partial header read)
        drop(file);

        let text = tokio::fs::read_to_string(file_path)
            .await
            .map_err(|e| format!("Error reading text file: {}", e))?;

        Ok(vec![ContentBlock::text(text)])
    }
}

/// Create the read tool definition
pub fn create_read_tool() -> AgentTool {
    let params = pick_ai::types::JsonSchema {
        schema_type: "object".to_string(),
        properties: Some(
            vec![
                (
                    "path".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Path to the file to read (relative or absolute, required)"
                    }),
                ),
                (
                    "offset".to_string(),
                    serde_json::json!({
                        "type": "number",
                        "description": "Line number to start reading from (1-indexed)"
                    }),
                ),
                (
                    "limit".to_string(),
                    serde_json::json!({
                        "type": "number",
                        "description": "Maximum number of lines to read"
                    }),
                ),
            ]
            .into_iter()
            .collect(),
        ),
        required: Some(vec!["path".to_string()]),
        description: Some("Read the contents of a file from a given path. Example: read(path: \"src/main.rs\", limit: 50)".to_string()),
        items: None,
        additional_properties: Some(false),
    };

    AgentTool {
        name: "read".to_string(),
        description: "Read the contents of a file from a given path. Example: read(path: \"src/main.rs\", limit: 50)".to_string(),
        prompt_snippet: Some("Read file contents".to_string()),
        prompt_guidelines: vec!["Use read to examine files instead of cat or sed.".to_string()],
        usage_example: Some(vec!["read(path: \"src/main.rs\", limit: 50)".to_string()]),
        label: "read".to_string(),
        parameters: params,
        execute: std::sync::Arc::new(|_tool_call_id, args, ctx| {
            Box::pin(async move {
                let file_path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing path argument".to_string())?;

                if let (Some(ref policy), Some(ref cwd)) = (ctx.fs_policy, ctx.cwd)
                    && let Err(_e) = policy.can_read(std::path::Path::new(file_path), cwd) {
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
                                "Read", file_path, pm, ctx.question.as_ref(), ctx.tool_event_bus.as_ref(),
                                ctx.tool_execution_permission.as_deref().unwrap_or("prompt"),
                            ).await?;
                            if !authorized {
                                return Ok(AgentToolResult {
                                    content: vec![ContentBlock::text(format!(
                                        "Error: Read access denied: '{}' is outside the allowed workspace", file_path
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

                let content_blocks = read_file(file_path).await?;

                // Apply offset/limit for text content
                let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let limit = args.get("limit").and_then(|v| v.as_u64());

                if (offset > 0 || limit.is_some()) && content_blocks.len() == 1
                    && let ContentBlock::Text(ref tc) = content_blocks[0] {
                        let lines: Vec<&str> = tc.text.split('\n').collect();
                        let start = offset.saturating_sub(1);
                        let selected: Vec<&str> = if let Some(lim) = limit {
                            lines.iter().copied().skip(start).take(lim as usize).collect()
                        } else {
                            lines.iter().copied().skip(start).collect()
                        };
                        return Ok(AgentToolResult {
                            content: vec![ContentBlock::text(selected.join("\n"))],
                            is_error: false,
                            terminate: false,
                        });
                    }

                Ok(AgentToolResult {
                    content: content_blocks,
                    is_error: false,
                    terminate: false,
                })
            })
        }),
        execution_mode: ToolExecutionMode::Parallel,
    }
}
