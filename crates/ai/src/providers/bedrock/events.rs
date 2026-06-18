use crate::types::{
    AssistantMessage, ContentBlock, StopReason, StreamEvent,
};

async fn handle_content_block_start(
    data: &serde_json::Value,
    tx: &tokio::sync::mpsc::Sender<StreamEvent>,
    output: &mut AssistantMessage,
    tool_blocks: &mut std::collections::HashMap<usize, (String, String, String)>,
) -> bool {
    if let Some(cbs) = data.get("contentBlockStart") {
        let block_index = cbs.get("contentBlockIndex").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        if let Some(start) = cbs.get("start") {
            if let Some(tool_use) = start.get("toolUse") {
                let tool_id = tool_use.get("toolUseId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let tool_name = tool_use.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                tool_blocks.insert(block_index, (tool_id.clone(), tool_name.clone(), String::new()));

                let content_idx = output.content.len();
                output.content.push(ContentBlock::tool_call(tool_id, tool_name, serde_json::Value::Null));

                let _ = tx.send(StreamEvent::ToolCallStart {
                    content_index: content_idx,
                    partial: super::partial_from_output(output),
                }).await;
                return true;
            }
        }
        return true;
    }
    false
}

async fn handle_content_block_delta(
    data: &serde_json::Value,
    tx: &tokio::sync::mpsc::Sender<StreamEvent>,
    output: &mut AssistantMessage,
    text_blocks: &mut std::collections::HashMap<usize, usize>,
    tool_blocks: &mut std::collections::HashMap<usize, (String, String, String)>,
) -> bool {
    if let Some(cbd) = data.get("contentBlockDelta") {
        let block_index = cbd.get("contentBlockIndex").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        if let Some(delta) = cbd.get("delta") {
            if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                let content_idx = if let Some(idx) = text_blocks.get(&block_index) {
                    *idx
                } else {
                    let idx = output.content.len();
                    output.content.push(ContentBlock::text(""));
                    text_blocks.insert(block_index, idx);
                    let _ = tx.send(StreamEvent::TextStart {
                        content_index: idx,
                        partial: super::partial_from_output(output),
                    }).await;
                    idx
                };
                if content_idx < output.content.len() {
                    if let ContentBlock::Text(ref mut tc) = output.content[content_idx] {
                        tc.text.push_str(text);
                    }
                    let _ = tx.send(StreamEvent::TextDelta {
                        content_index: content_idx,
                        delta: text.to_string(),
                        partial: super::partial_from_output(output),
                    }).await;
                }
                return true;
            }

            if let Some(tool_use) = delta.get("toolUse") {
                if let Some(input) = tool_use.get("input").and_then(|v| v.as_str()) {
                    if let Some((_, _, partial)) = tool_blocks.get_mut(&block_index) {
                        partial.push_str(input);
                        let partial_str = partial.clone();
                        if let Some(idx) = super::find_tool_call_index(output, block_index, tool_blocks) {
                            let args: serde_json::Value = serde_json::from_str(&partial_str).unwrap_or(serde_json::Value::Null);
                            if let ContentBlock::ToolCall(ref mut tc) = output.content[idx] {
                                tc.arguments = args;
                            }
                            let _ = tx.send(StreamEvent::ToolCallDelta {
                                content_index: idx,
                                delta: input.to_string(),
                                partial: super::partial_from_output(output),
                            }).await;
                        }
                    }
                }
                return true;
            }

            if let Some(reasoning) = delta.get("reasoningContent") {
                if let Some(reasoning_text) = reasoning.get("reasoningText") {
                    if let Some(text) = reasoning_text.get("text").and_then(|v| v.as_str()) {
                        let thinking_idx = output.content.iter().position(|c| matches!(c, ContentBlock::Thinking(_)));
                        let content_idx = if let Some(idx) = thinking_idx {
                            idx
                        } else {
                            let idx = output.content.len();
                            output.content.push(ContentBlock::Thinking(crate::types::ThinkingContent {
                                thinking: String::new(),
                                thinking_signature: None,
                                redacted: false,
                            }));
                            let _ = tx.send(StreamEvent::ThinkingStart {
                                content_index: idx,
                                partial: super::partial_from_output(output),
                            }).await;
                            idx
                        };
                        if let ContentBlock::Thinking(ref mut tc) = output.content[content_idx] {
                            tc.thinking.push_str(text);
                        }
                        if let Some(sig) = reasoning_text.get("signature").and_then(|v| v.as_str()) {
                            if let ContentBlock::Thinking(ref mut tc) = output.content[content_idx] {
                                tc.thinking_signature = Some(sig.to_string());
                            }
                        }
                        let _ = tx.send(StreamEvent::ThinkingDelta {
                            content_index: content_idx,
                            delta: text.to_string(),
                            partial: super::partial_from_output(output),
                        }).await;
                        return true;
                    }
                }
            }
        }
        return true;
    }
    false
}

fn handle_message_stop(
    data: &serde_json::Value,
    output: &mut AssistantMessage,
) -> bool {
    if let Some(ms) = data.get("messageStop") {
        if let Some(reason) = ms.get("stopReason").and_then(|v| v.as_str()) {
            output.stop_reason = match reason {
                "end_turn" | "stop_sequence" => StopReason::Stop,
                "max_tokens" | "model_context_window_exceeded" => StopReason::Length,
                "tool_use" => StopReason::ToolUse,
                _ => StopReason::Stop,
            };
        }
        return true;
    }
    false
}

pub(crate) async fn process_bedrock_event(
    tx: &tokio::sync::mpsc::Sender<StreamEvent>,
    sse: &crate::sse::SseEvent,
    output: &mut AssistantMessage,
    text_blocks: &mut std::collections::HashMap<usize, usize>,
    tool_blocks: &mut std::collections::HashMap<usize, (String, String, String)>,
    saw_message_start: &mut bool,
) {
    let data: serde_json::Value = match serde_json::from_str(&sse.data) {
        Ok(v) => v,
        Err(_) => return,
    };

    if let Some(start) = data.get("messageStart") {
        *saw_message_start = true;
        if let Some(role) = start.get("role").and_then(|v| v.as_str()) {
            if role == "assistant" {
            }
        }
        let _ = tx.send(StreamEvent::Start {
            partial: super::partial_from_output(output),
        }).await;
        return;
    }

    if handle_content_block_start(&data, tx, output, tool_blocks).await {
        return;
    }

    if handle_content_block_delta(&data, tx, output, text_blocks, tool_blocks).await {
        return;
    }

    if data.get("contentBlockStop").is_some() {
        let block_index = data.get("contentBlockStop")
            .and_then(|v| v.get("contentBlockIndex"))
            .and_then(|v| v.as_u64()).unwrap_or(0) as usize;

        if let Some((_, _, partial_json)) = tool_blocks.get(&block_index) {
            if let Some(idx) = super::find_tool_call_index(output, block_index, tool_blocks) {
                let args: serde_json::Value = serde_json::from_str(partial_json).unwrap_or(serde_json::Value::Null);
                if let ContentBlock::ToolCall(ref mut tc) = output.content[idx] {
                    tc.arguments = args;
                }
                if let ContentBlock::ToolCall(tc) = &output.content[idx] {
                    let _ = tx.send(StreamEvent::ToolCallEnd {
                        content_index: idx,
                        tool_call: tc.clone(),
                        partial: super::partial_from_output(output),
                    }).await;
                }
            }
            return;
        }

        if let Some(idx) = text_blocks.get(&block_index) {
            if *idx < output.content.len() {
                if let ContentBlock::Text(ref tc) = output.content[*idx] {
                    let _ = tx.send(StreamEvent::TextEnd {
                        content_index: *idx,
                        content: tc.text.clone(),
                        partial: super::partial_from_output(output),
                    }).await;
                }
            }
            return;
        }

        let thinking_idx = output.content.iter().position(|c| matches!(c, ContentBlock::Thinking(_)));
        if let Some(idx) = thinking_idx {
            if let ContentBlock::Thinking(ref tc) = output.content[idx] {
                let _ = tx.send(StreamEvent::ThinkingEnd {
                    content_index: idx,
                    content: tc.thinking.clone(),
                    partial: super::partial_from_output(output),
                }).await;
            }
        }
        return;
    }

    if handle_message_stop(&data, output) {
        return;
    }

    if let Some(metadata) = data.get("metadata") {
        if let Some(usage) = metadata.get("usage") {
            if let Some(val) = usage.get("inputTokens").and_then(|v| v.as_u64()) {
                output.usage.input = val;
            }
            if let Some(val) = usage.get("outputTokens").and_then(|v| v.as_u64()) {
                output.usage.output = val;
            }
            if let Some(val) = usage.get("cacheReadInputTokens").and_then(|v| v.as_u64()) {
                output.usage.cache_read = val;
            }
            if let Some(val) = usage.get("cacheWriteInputTokens").and_then(|v| v.as_u64()) {
                output.usage.cache_write = val;
            }
            output.usage.total_tokens = output.usage.input + output.usage.output + output.usage.cache_read + output.usage.cache_write;
        }
        if let Some(id) = metadata.get("requestId").or_else(|| metadata.get("request_id")).and_then(|v| v.as_str()) {
            if output.response_id.is_none() {
                output.response_id = Some(id.to_string());
            }
        }
        return;
    }

    if data.get("internalServerException").is_some() {
        output.stop_reason = StopReason::Error;
        output.error_message = Some("Bedrock internal server error".to_string());
        let _ = tx.send(StreamEvent::Error {
            reason: StopReason::Error,
            error: output.clone(),
        }).await;
        return;
    }

    for error_field in &["modelStreamErrorException", "validationException", "throttlingException", "serviceUnavailableException"] {
        if let Some(error) = data.get(*error_field) {
            output.stop_reason = StopReason::Error;
            let msg = error.get("message").and_then(|v| v.as_str()).unwrap_or(*error_field);
            output.error_message = Some(format!("Bedrock {}: {}", error_field, msg));
            let _ = tx.send(StreamEvent::Error {
                reason: StopReason::Error,
                error: output.clone(),
            }).await;
            return;
        }
    }
}
