use crate::sse::SseEvent;
use crate::types::{AssistantMessage, ContentBlock, StreamEvent};

/// Block tracking state for content blocks during streaming
pub(crate) struct BlockState {
    pub(crate) block_type: String,
    pub(crate) index: usize,
    pub(crate) content_index: usize,
    pub(crate) text: String,
    pub(crate) thinking_signature: Option<String>,
    pub(crate) partial_json: String,
    pub(crate) arguments: serde_json::Value,
}

fn handle_anthropic_ping(sse: &SseEvent) -> Result<(), String> {
    if sse.event.as_deref() == Some("error") {
        return Err(sse.data.clone());
    }
    Ok(())
}

fn handle_anthropic_message_start(
    data: &serde_json::Value,
    output: &mut AssistantMessage,
    saw_message_start: &mut bool,
) {
    *saw_message_start = true;
    if let Some(msg) = data.get("message") {
        if let Some(id) = msg.get("id").and_then(|v| v.as_str()) {
            output.response_id = Some(id.to_string());
        }
        if let Some(usage) = msg.get("usage") {
            output.usage.input = usage
                .get("input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            output.usage.output = usage
                .get("output_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            output.usage.cache_read = usage
                .get("cache_read_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            output.usage.cache_write = usage
                .get("cache_creation_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            output.usage.total_tokens = output.usage.input
                + output.usage.output
                + output.usage.cache_read
                + output.usage.cache_write;
        }
    }
}

async fn handle_anthropic_content_block_start(
    data: &serde_json::Value,
    output: &mut AssistantMessage,
    blocks: &mut Vec<BlockState>,
    tx: &tokio::sync::mpsc::Sender<StreamEvent>,
) -> Result<(), String> {
    let index = data.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let cb = data
        .get("content_block")
        .ok_or_else(|| "Missing content_block".to_string())?;
    let block_type_str = cb.get("type").and_then(|v| v.as_str()).unwrap_or("text");

    match block_type_str {
        "text" => {
            let content_index = output.content.len();
            let block = BlockState {
                block_type: "text".to_string(),
                index,
                content_index,
                text: String::new(),
                thinking_signature: None,
                partial_json: String::new(),
                arguments: serde_json::Value::Null,
            };
            output.content.push(ContentBlock::text(""));
            blocks.push(block);
            let _ = tx
                .send(StreamEvent::TextStart {
                    content_index,
                    partial: super::partial_from_output(output),
                })
                .await;
        }
        "thinking" => {
            let content_index = output.content.len();
            let block = BlockState {
                block_type: "thinking".to_string(),
                index,
                content_index,
                text: String::new(),
                thinking_signature: None,
                partial_json: String::new(),
                arguments: serde_json::Value::Null,
            };
            output
                .content
                .push(ContentBlock::Thinking(crate::types::ThinkingContent {
                    thinking: String::new(),
                    thinking_signature: None,
                    redacted: false,
                }));
            blocks.push(block);
            let _ = tx
                .send(StreamEvent::ThinkingStart {
                    content_index,
                    partial: super::partial_from_output(output),
                })
                .await;
        }
        "redacted_thinking" => {
            let data_val = cb.get("data").and_then(|v| v.as_str()).unwrap_or("");
            let content_index = output.content.len();
            let block = BlockState {
                block_type: "thinking".to_string(),
                index,
                content_index,
                text: "[Reasoning redacted]".to_string(),
                thinking_signature: Some(data_val.to_string()),
                partial_json: String::new(),
                arguments: serde_json::Value::Null,
            };
            output
                .content
                .push(ContentBlock::Thinking(crate::types::ThinkingContent {
                    thinking: "[Reasoning redacted]".to_string(),
                    thinking_signature: Some(data_val.to_string()),
                    redacted: true,
                }));
            blocks.push(block);
            let _ = tx
                .send(StreamEvent::ThinkingStart {
                    content_index,
                    partial: super::partial_from_output(output),
                })
                .await;
        }
        "tool_use" => {
            let id = cb.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let name = cb.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let input = cb.get("input").unwrap_or(&serde_json::Value::Null);
            let is_placeholder =
                input.is_null() || input.as_object().map_or(false, |o| o.is_empty());
            let content_index = output.content.len();
            let block = BlockState {
                block_type: "toolCall".to_string(),
                index,
                content_index,
                text: String::new(),
                thinking_signature: None,
                partial_json: if is_placeholder {
                    String::new()
                } else {
                    input.to_string()
                },
                arguments: if is_placeholder {
                    serde_json::Value::Object(Default::default())
                } else {
                    input.clone()
                },
            };
            output
                .content
                .push(ContentBlock::tool_call(id, name, input.clone()));
            blocks.push(block);
            let _ = tx
                .send(StreamEvent::ToolCallStart {
                    content_index,
                    partial: super::partial_from_output(output),
                })
                .await;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_anthropic_content_block_delta(
    data: &serde_json::Value,
    output: &mut AssistantMessage,
    blocks: &mut Vec<BlockState>,
    tx: &tokio::sync::mpsc::Sender<StreamEvent>,
) -> Result<(), String> {
    let index = data.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let delta = data
        .get("delta")
        .ok_or_else(|| "Missing delta".to_string())?;
    let delta_type = delta.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match delta_type {
        "text_delta" => {
            let text = delta.get("text").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(block) = blocks
                .iter_mut()
                .find(|b| b.index == index && b.block_type == "text")
            {
                block.text += text;
                let idx = block.content_index;
                if idx < output.content.len() {
                    if let ContentBlock::Text(ref mut tc) = output.content[idx] {
                        tc.text = block.text.clone();
                    }
                    let _ = tx
                        .send(StreamEvent::TextDelta {
                            content_index: idx,
                            delta: text.to_string(),
                            partial: super::partial_from_output(output),
                        })
                        .await;
                }
            }
        }
        "thinking_delta" => {
            let thinking = delta.get("thinking").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(block) = blocks
                .iter_mut()
                .find(|b| b.index == index && b.block_type == "thinking")
            {
                block.text += thinking;
                let idx = block.content_index;
                if idx < output.content.len() {
                    if let ContentBlock::Thinking(ref mut tc) = output.content[idx] {
                        tc.thinking = block.text.clone();
                    }
                    let _ = tx
                        .send(StreamEvent::ThinkingDelta {
                            content_index: idx,
                            delta: thinking.to_string(),
                            partial: super::partial_from_output(output),
                        })
                        .await;
                }
            }
        }
        "input_json_delta" => {
            let partial = delta
                .get("partial_json")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if let Some(block) = blocks
                .iter_mut()
                .find(|b| b.index == index && b.block_type == "toolCall")
            {
                block.partial_json += partial;
                block.arguments = super::try_parse_json(&block.partial_json);
                let idx = block.content_index;
                if idx < output.content.len() {
                    if let ContentBlock::ToolCall(ref mut tc) = output.content[idx] {
                        tc.arguments = block.arguments.clone();
                    }
                    let _ = tx
                        .send(StreamEvent::ToolCallDelta {
                            content_index: idx,
                            delta: partial.to_string(),
                            partial: super::partial_from_output(output),
                        })
                        .await;
                }
            }
        }
        "signature_delta" => {
            let signature = delta
                .get("signature")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if let Some(block) = blocks
                .iter_mut()
                .find(|b| b.index == index && b.block_type == "thinking")
            {
                block.thinking_signature =
                    Some(block.thinking_signature.clone().unwrap_or_default() + signature);
                let idx = block.content_index;
                if idx < output.content.len() {
                    if let ContentBlock::Thinking(ref mut tc) = output.content[idx] {
                        tc.thinking_signature = block.thinking_signature.clone();
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_anthropic_message_delta(data: &serde_json::Value, output: &mut AssistantMessage) {
    if let Some(delta) = data.get("delta") {
        if let Some(stop_reason) = delta.get("stop_reason").and_then(|v| v.as_str()) {
            output.stop_reason = super::map_anthropic_stop_reason(stop_reason);
        }
    }
    if let Some(usage) = data.get("usage") {
        if let Some(val) = usage.get("input_tokens").and_then(|v| v.as_u64()) {
            output.usage.input = val;
        }
        if let Some(val) = usage.get("output_tokens").and_then(|v| v.as_u64()) {
            output.usage.output = val;
        }
        if let Some(val) = usage
            .get("cache_read_input_tokens")
            .and_then(|v| v.as_u64())
        {
            output.usage.cache_read = val;
        }
        if let Some(val) = usage
            .get("cache_creation_input_tokens")
            .and_then(|v| v.as_u64())
        {
            output.usage.cache_write = val;
        }
        output.usage.total_tokens = output.usage.input
            + output.usage.output
            + output.usage.cache_read
            + output.usage.cache_write;
    }
}

fn handle_anthropic_message_stop(saw_message_stop: &mut bool) {
    *saw_message_stop = true;
}

pub(crate) async fn process_anthropic_event(
    tx: &tokio::sync::mpsc::Sender<StreamEvent>,
    sse: SseEvent,
    output: &mut AssistantMessage,
    blocks: &mut Vec<BlockState>,
    saw_message_start: &mut bool,
    saw_message_stop: &mut bool,
    _stream_error: &mut Option<String>,
) -> Result<(), String> {
    handle_anthropic_ping(&sse)?;

    let data: serde_json::Value = serde_json::from_str(&sse.data)
        .map_err(|e| format!("Failed to parse SSE data: {}; data={}", e, sse.data))?;

    let event_type = data
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Missing type in SSE event: {}", sse.data))?;

    match event_type {
        "message_start" => {
            handle_anthropic_message_start(&data, output, saw_message_start);
        }

        "content_block_start" => {
            handle_anthropic_content_block_start(&data, output, blocks, tx).await?;
        }

        "content_block_delta" => {
            handle_anthropic_content_block_delta(&data, output, blocks, tx).await?;
        }

        "content_block_stop" => {
            let index = data.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            if let Some(block) = blocks.iter().find(|b| b.index == index) {
                let idx = block.content_index;
                match block.block_type.as_str() {
                    "text" => {
                        if idx < output.content.len() {
                            let _ = tx
                                .send(StreamEvent::TextEnd {
                                    content_index: idx,
                                    content: block.text.clone(),
                                    partial: super::partial_from_output(output),
                                })
                                .await;
                        }
                    }
                    "thinking" => {
                        if idx < output.content.len() {
                            let _ = tx
                                .send(StreamEvent::ThinkingEnd {
                                    content_index: idx,
                                    content: block.text.clone(),
                                    partial: super::partial_from_output(output),
                                })
                                .await;
                        }
                    }
                    "toolCall" => {
                        if idx < output.content.len() {
                            let args = if block.partial_json.is_empty() {
                                block.arguments.clone()
                            } else {
                                serde_json::from_str(&block.partial_json)
                                    .unwrap_or_else(|_| block.arguments.clone())
                            };
                            if let ContentBlock::ToolCall(ref mut tc) = output.content[idx] {
                                tc.arguments = args;
                            }
                            if let Some(tc) = output.content.get(idx).and_then(|c| {
                                if let ContentBlock::ToolCall(tc) = c {
                                    Some(tc.clone())
                                } else {
                                    None
                                }
                            }) {
                                let _ = tx
                                    .send(StreamEvent::ToolCallEnd {
                                        content_index: idx,
                                        tool_call: tc,
                                        partial: super::partial_from_output(output),
                                    })
                                    .await;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        "message_delta" => {
            handle_anthropic_message_delta(&data, output);
        }

        "message_stop" => {
            handle_anthropic_message_stop(saw_message_stop);
        }

        _ => {}
    }

    Ok(())
}
