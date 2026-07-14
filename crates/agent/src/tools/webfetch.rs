//! WebFetch tool - fetches content from URLs

use std::time::Duration;

use pick_ai::types::ContentBlock;
use scraper::Element;

use crate::core::hooks::{ToolEvent, WaitingKind};
use crate::core::state::{AgentTool, AgentToolResult, ToolContext, ToolExecutionMode};

const MAX_RESPONSE_SIZE: usize = 5 * 1024 * 1024;
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_TIMEOUT_SECS: u64 = 120;

/// Convert HTML to plain text by extracting visible text content
fn html_to_text(html: &str) -> String {
    let fragment = scraper::Html::parse_fragment(html);
    let root = fragment.root_element();
    let mut text = String::new();
    for node in root.text() {
        let trimmed = node.trim();
        if !trimmed.is_empty() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(trimmed);
        }
    }
    text
}

/// Simple HTML to Markdown conversion for LLM consumption
fn html_to_markdown(html: &str) -> String {
    let document = scraper::Html::parse_document(html);
    let mut md = String::new();
    convert_node_to_markdown(&document.root_element(), &mut md, 0);
    md
}

fn convert_node_to_markdown(node: &scraper::ElementRef, md: &mut String, depth: usize) {
    for child in node.child_elements() {
        let tag = child.value().name();
        match tag {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let level = tag[1..].parse::<usize>().unwrap_or(1);
                let prefix = "#".repeat(level);
                if !md.is_empty() && !md.ends_with('\n') {
                    md.push('\n');
                }
                md.push_str(&format!("{} ", prefix));
                for text in child.text() {
                    md.push_str(text.trim());
                }
                md.push_str("\n\n");
            }
            "p" | "div" | "span" | "article" | "section" | "header" | "footer" | "main" | "nav" => {
                for text in child.text() {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        md.push_str(trimmed);
                        md.push(' ');
                    }
                }
                if tag == "p" {
                    md.push_str("\n\n");
                }
                convert_node_to_markdown(&child, md, depth);
            }
            "a" => {
                let href = child.attr("href").unwrap_or("");
                let mut text = String::new();
                for t in child.text() {
                    text.push_str(t.trim());
                }
                if !text.is_empty() {
                    md.push_str(&format!("[{}]({})", text, href));
                }
            }
            "img" => {
                let src = child.attr("src").unwrap_or("");
                let alt = child.attr("alt").unwrap_or("");
                md.push_str(&format!("![{}]({})", alt, src));
            }
            "code" => {
                md.push('`');
                for t in child.text() {
                    md.push_str(t.trim());
                }
                md.push('`');
            }
            "pre" => {
                md.push_str("\n```\n");
                for t in child.text() {
                    md.push_str(t);
                }
                if !md.ends_with('\n') {
                    md.push('\n');
                }
                md.push_str("```\n\n");
            }
            "ul" | "ol" => {
                convert_node_to_markdown(&child, md, depth + 1);
                md.push('\n');
            }
            "li" => {
                let prefix = if child
                    .parent_element()
                    .is_some_and(|p| p.value().name() == "ol")
                {
                    "1. "
                } else {
                    "- "
                };
                md.push_str(&"  ".repeat(depth));
                md.push_str(prefix);
                for t in child.text() {
                    md.push_str(t.trim());
                }
                md.push('\n');
                convert_node_to_markdown(&child, md, depth + 1);
            }
            "br" => md.push('\n'),
            "hr" | "hr/" => md.push_str("\n---\n\n"),
            "blockquote" => {
                for t in child.text() {
                    let trimmed = t.trim();
                    if !trimmed.is_empty() {
                        md.push_str(&format!("> {}\n", trimmed));
                    }
                }
                md.push('\n');
            }
            "strong" | "b" => {
                md.push_str("**");
                for t in child.text() {
                    md.push_str(t.trim());
                }
                md.push_str("**");
            }
            "em" | "i" => {
                md.push('*');
                for t in child.text() {
                    md.push_str(t.trim());
                }
                md.push('*');
            }
            _ => {
                convert_node_to_markdown(&child, md, depth);
            }
        }
    }
}

/// Check if a MIME type is an image (except SVG)
fn is_image_mime(mime: &str) -> bool {
    let mime = mime.to_lowercase();
    (mime.starts_with("image/") && !mime.contains("svg")) || mime == "application/octet-stream"
}

/// Create the webfetch tool
pub fn create_webfetch_tool() -> AgentTool {
    let params = pick_ai::types::JsonSchema {
        schema_type: "object".to_string(),
        properties: Some(
            vec![
                (
                    "url".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "The URL to fetch content from (must start with http:// or https://)"
                    }),
                ),
                (
                    "format".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "enum": ["markdown", "text", "html"],
                        "description": "The format to return the content in (markdown, text, or html). Defaults to markdown."
                    }),
                ),
                (
                    "timeout".to_string(),
                    serde_json::json!({
                        "type": "number",
                        "description": "Optional timeout in seconds (max 120)"
                    }),
                ),
            ]
            .into_iter()
            .collect(),
        ),
        required: Some(vec!["url".to_string()]),
        description: Some("Fetch content from a URL and return it in markdown, text, or html format. Use this tool when you need to retrieve and analyze web content.".to_string()),
        items: None,
        additional_properties: Some(false),
    };

    AgentTool {
        name: "webfetch".to_string(),
        description: "Fetch content from a URL and return it in markdown, text, or html format. Use this tool when you need to retrieve and analyze web content.".to_string(),
        prompt_snippet: Some("Fetch web content from URLs".to_string()),
        prompt_guidelines: vec![],
        usage_example: Some(vec!["webfetch(url: \"https://example.com\")".to_string()]),
        label: "webfetch".to_string(),
        parameters: params,
        execute: std::sync::Arc::new(move |tool_call_id, args, ctx: ToolContext| {
            Box::pin(async move {
                let url = args
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing 'url' argument".to_string())?;

                if !url.starts_with("http://") && !url.starts_with("https://") {
                    return Err("URL must start with http:// or https://".to_string());
                }

                // Check network policy if available
                if let Some(ref pm) = ctx.permission_manager {
                    use crate::permission::network::NetworkDenyReason;
                    match pm.check_network_detailed(url) {
                        Ok(()) => {} // allowed
                        Err(NetworkDenyReason::Blocked(e)) => {
                            return Err(format!("NetworkPolicy: {}", e));
                        }
                        Err(NetworkDenyReason::NotAllowed(e)) => {
                            // Not in allowlist — prompt user for permission
                            if let Some(ref approve) = ctx.approve {
                                if let Some(ref bus) = ctx.tool_event_bus {
                                    bus.publish(&ToolEvent::WaitingForUser {
                                        tool_name: "webfetch".to_string(),
                                        tool_call_id: tool_call_id.to_string(),
                                        input: args.clone(),
                                        kind: WaitingKind::Permission {
                                            permission: "webfetch".to_string(),
                                        },
                                        summary: format!("Fetch URL '{}'", url),
                                    })
                                    .await;
                                }
                                if !approve("webfetch".to_string(), url.to_string()).await {
                                    return Ok(AgentToolResult {
                                        content: vec![ContentBlock::text(
                                            "Permission denied for webfetch",
                                        )],
                                        is_error: true,
                                        terminate: false,
                                    });
                                }
                            } else {
                                return Err(format!("NetworkPolicy: {}", e));
                            }
                        }
                    }
                } else {
                    // No network policy configured — ask user for permission
                    if let Some(ref approve) = ctx.approve {
                        if let Some(ref bus) = ctx.tool_event_bus {
                            bus.publish(&ToolEvent::WaitingForUser {
                                tool_name: "webfetch".to_string(),
                                tool_call_id: tool_call_id.to_string(),
                                input: args.clone(),
                                kind: WaitingKind::Permission {
                                    permission: "webfetch".to_string(),
                                },
                                summary: format!("Fetch URL '{}'", url),
                            })
                            .await;
                        }
                        if !approve("webfetch".to_string(), url.to_string()).await {
                            return Ok(AgentToolResult {
                                content: vec![ContentBlock::text("Permission denied for webfetch")],
                                is_error: true,
                                terminate: false,
                            });
                        }
                    }
                }

                let format = args
                    .get("format")
                    .and_then(|v| v.as_str())
                    .unwrap_or("markdown")
                    .to_string();

                let timeout_secs = args
                    .get("timeout")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(DEFAULT_TIMEOUT_SECS)
                    .min(MAX_TIMEOUT_SECS);

                let client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(timeout_secs))
                    .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                    .redirect(reqwest::redirect::Policy::limited(10))
                    .build()
                    .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

                let response = client.get(url)
                    .header("Accept", "text/html,application/xhtml+xml,text/plain,*/*;q=0.8")
                    .header("Accept-Language", "en-US,en;q=0.9")
                    .send()
                    .await
                    .map_err(|e| format!("HTTP request failed: {}", e))?;

                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("application/octet-stream")
                    .to_string();

                // Check content-length header
                if let Some(len) = response.content_length()
                    && len > MAX_RESPONSE_SIZE as u64 {
                        return Ok(AgentToolResult {
                            content: vec![ContentBlock::text("Error: Response too large (exceeds 5MB limit)")],
                            is_error: true,
                            terminate: false,
                        });
                    }

                let bytes = response.bytes().await
                    .map_err(|e| format!("Failed to read response body: {}", e))?;

                if bytes.len() > MAX_RESPONSE_SIZE {
                    return Ok(AgentToolResult {
                        content: vec![ContentBlock::text("Error: Response too large (exceeds 5MB limit)")],
                        is_error: true,
                        terminate: false,
                    });
                }

                let mime = content_type.split(';').next().unwrap_or("").trim().to_lowercase();

                // Handle images
                if is_image_mime(&mime) {
                    use base64::Engine;
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                    return Ok(AgentToolResult {
                        content: vec![ContentBlock::image(b64, mime)],
                        is_error: false,
                        terminate: false,
                    });
                }

                let body = String::from_utf8_lossy(&bytes).to_string();

                let is_html = mime.contains("html") || mime.contains("xhtml");

                let output = match format.as_str() {
                    "html" => body,
                    "text" => {
                        if is_html { html_to_text(&body) } else { body }
                    }
                    _ => {
                        // Default: markdown
                        if is_html { html_to_markdown(&body) } else { body }
                    }
                };

                let title = format!("{} ({})", url, content_type);

                Ok(AgentToolResult {
                    content: vec![ContentBlock::text(format!("{}\n\n{}", title, output))],
                    is_error: false,
                    terminate: false,
                })
            })
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}
