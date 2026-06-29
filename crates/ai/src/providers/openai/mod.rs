//! OpenAI Chat Completions API provider with SSE streaming

pub mod stream;
pub use stream::*;

use crate::types::{ContentBlock, Message, Model};

/// Check if a model requires `reasoning_content` on replayed assistant messages
/// (needed for DeepSeek multi-turn conversations).
fn requires_reasoning_content_on_assistant_messages(model: Option<&Model>) -> bool {
    model
        .and_then(|m| m.compat.as_ref())
        .and_then(|c| c.openai_completions.as_ref())
        .and_then(|oc| oc.requires_reasoning_content_on_assistant_messages)
        .unwrap_or(false)
}

/// Check if a model requires thinking blocks to be sent as plain text
/// (rather than as a separate reasoning_content field).
fn requires_thinking_as_text(model: Option<&Model>) -> bool {
    model
        .and_then(|m| m.compat.as_ref())
        .and_then(|c| c.openai_completions.as_ref())
        .and_then(|oc| oc.requires_thinking_as_text)
        .unwrap_or(false)
}

/// Convert Pick internal messages to OpenAI Chat Completions API format.
/// Optionally accepts a model reference to enable provider-specific behavior
/// such as `reasoning_content` on assistant messages for DeepSeek multi-turn.
pub fn convert_to_openai_messages(
    messages: &[Message],
    model: Option<&Model>,
) -> Vec<serde_json::Value> {
    let needs_reasoning_content = requires_reasoning_content_on_assistant_messages(model);
    let thinking_as_text = requires_thinking_as_text(model);

    messages
        .iter()
        .map(|msg| match msg {
            Message::Assistant(a) => {
                let mut text_parts: Vec<String> = Vec::new();
                let mut tool_calls: Vec<serde_json::Value> = Vec::new();
                let mut reasoning_content: Option<String> = None;

                for block in &a.content {
                    match block {
                        ContentBlock::Text(t) => text_parts.push(t.text.clone()),
                        ContentBlock::Thinking(t) if !t.thinking.is_empty() => {
                            if needs_reasoning_content {
                                // Send thinking content as reasoning_content field (e.g. DeepSeek)
                                reasoning_content = Some(t.thinking.clone());
                                if thinking_as_text {
                                    text_parts.push(t.thinking.clone());
                                }
                            } else {
                                // Default: convert to plain text
                                text_parts.push(t.thinking.clone());
                            }
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
                    serde_json::Value::String(String::new())
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
                // DeepSeek requires reasoning_content on all assistant messages in multi-turn
                if needs_reasoning_content {
                    msg_json["reasoning_content"] =
                        serde_json::Value::String(reasoning_content.unwrap_or_default());
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
    let stream_opts = options.map(|o| {
        let mut opts = o.base;
        // Preserve the reasoning level from SimpleStreamOptions into StreamOptions
        // so it can be used by build_thinking_params() in the streaming implementation.
        if o.reasoning.is_some() {
            opts.reasoning = o.reasoning;
        }
        opts
    });
    stream::stream_openai_completions(model, context, stream_opts)
}
