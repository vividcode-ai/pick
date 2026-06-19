//! OpenAI Chat Completions API provider with SSE streaming

pub mod stream;
pub use stream::*;

use crate::types::{ContentBlock, Message};

/// Convert Pick internal messages to OpenAI Chat Completions API format.
pub fn convert_to_openai_messages(messages: &[Message]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .map(|msg| match msg {
            Message::Assistant(a) => {
                let mut text_parts: Vec<String> = Vec::new();
                let mut tool_calls: Vec<serde_json::Value> = Vec::new();

                for block in &a.content {
                    match block {
                        ContentBlock::Text(t) => text_parts.push(t.text.clone()),
                        ContentBlock::Thinking(t) if !t.thinking.is_empty() => {
                            text_parts.push(t.thinking.clone());
                        }
                        ContentBlock::ToolCall(tc) => {
                            tool_calls.push(serde_json::json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    "arguments": tc.arguments.to_string(),
                                }
                            }));
                        }
                        _ => {}
                    }
                }

                let content = if text_parts.is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::Value::String(text_parts.join(""))
                };

                let mut msg_json = serde_json::json!({
                    "role": "assistant",
                    "content": content,
                });
                if !tool_calls.is_empty() {
                    msg_json["tool_calls"] = serde_json::json!(tool_calls);
                }
                msg_json
            }
            Message::ToolResult(tr) => {
                let content_text: String = tr
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        ContentBlock::Text(t) => Some(t.text.clone()),
                        ContentBlock::Thinking(t) => {
                            if t.thinking.is_empty() {
                                None
                            } else {
                                Some(t.thinking.clone())
                            }
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                serde_json::json!({
                    "role": "tool",
                    "tool_call_id": tr.tool_call_id,
                    "content": content_text,
                })
            }
            Message::User(u) => {
                let mut json_blocks: Vec<serde_json::Value> = Vec::new();
                for block in &u.content {
                    match block {
                        ContentBlock::Text(t) => {
                            json_blocks.push(serde_json::json!({
                                "type": "text",
                                "text": t.text,
                            }));
                        }
                        ContentBlock::Image(img) => {
                            json_blocks.push(serde_json::json!({
                                "type": "image_url",
                                "image_url": {
                                    "url": format!("data:{};base64,{}", img.mime_type, img.data),
                                }
                            }));
                        }
                        ContentBlock::Thinking(t) if !t.thinking.is_empty() => {
                            json_blocks.push(serde_json::json!({
                                "type": "text",
                                "text": t.thinking,
                            }));
                        }
                        _ => {}
                    }
                }

                if json_blocks.len() == 1 && json_blocks[0]["type"] == "text" {
                    serde_json::json!({
                        "role": "user",
                        "content": json_blocks[0]["text"],
                    })
                } else {
                    serde_json::json!({
                        "role": "user",
                        "content": json_blocks,
                    })
                }
            }
        })
        .collect()
}

/// Simple streaming version
pub fn stream_simple_openai_completions(
    model: crate::types::Model,
    context: crate::types::Context,
    options: Option<crate::types::SimpleStreamOptions>,
) -> tokio::sync::mpsc::Receiver<crate::types::StreamEvent> {
    let stream_opts = options.map(|o| o.base);
    stream::stream_openai_completions(model, context, stream_opts)
}
