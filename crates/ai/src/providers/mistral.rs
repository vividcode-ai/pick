//! Mistral Chat Completions API provider with SSE streaming
//! Mistral uses an OpenAI-compatible chat completions API

use crate::sse::SseParser;
use crate::types::{
    AssistantMessage, ContentBlock, Context, Model, StopReason, StreamEvent, StreamOptions, Usage,
};

/// Stream a response from Mistral's Chat Completions API
pub fn stream_mistral(
    model: Model,
    context: Context,
    options: Option<StreamOptions>,
) -> tokio::sync::mpsc::Receiver<StreamEvent> {
    let (tx, rx) = tokio::sync::mpsc::channel(64);

    let max_retries = options.as_ref().and_then(|o| o.max_retries).unwrap_or(3);
    let max_delay = options
        .as_ref()
        .and_then(|o| o.max_retry_delay_ms)
        .unwrap_or(60000);

    tokio::spawn(async move {
        let api_key = options
            .as_ref()
            .and_then(|o| o.api_key.clone())
            .or_else(|| std::env::var("MISTRAL_API_KEY").ok())
            .unwrap_or_default();

        if api_key.is_empty() {
            let mut msg = AssistantMessage::new(
                vec![],
                "mistral-conversations".to_string(),
                "mistral".to_string(),
                model.id.clone(),
                Usage::zero(),
                StopReason::Error,
            );
            msg.error_message = Some("MISTRAL_API_KEY is not set.".to_string());
            let _ = tx
                .send(StreamEvent::Error {
                    reason: StopReason::Error,
                    error: msg,
                })
                .await;
            return;
        }

        let url = format!(
            "{}/v1/chat/completions",
            model.base_url.trim_end_matches('/')
        );

        let messages: Vec<serde_json::Value> =
            crate::providers::openai::convert_to_openai_messages(&context.messages, None);

        let mut body = serde_json::json!({
            "model": model.id,
            "messages": messages,
            "stream": true,
        });
        if let Some(tools) = &context.tools {
            body["tools"] = serde_json::to_value(tools).unwrap_or_default();
        }
        if let Some(system) = &context.system_prompt {
            let mut all_messages = vec![serde_json::json!({"role": "system", "content": system})];
            if let Some(arr) = body["messages"].as_array() {
                all_messages.extend(arr.clone());
            }
            body["messages"] = serde_json::json!(all_messages);
        }

        // Apply reasoning_effort if model supports reasoning
        if model.reasoning
            && let Some(ref opts) = options
            && let Some(ref reasoning) = opts.reasoning
            && reasoning != "off"
        {
            let effort = model
                .thinking_level_map
                .as_ref()
                .and_then(|tlm| tlm.get(reasoning.as_str()))
                .and_then(|v| v.as_ref())
                .map(|s| s.as_str())
                .unwrap_or(reasoning.as_str());
            body["reasoning_effort"] = serde_json::Value::String(effort.to_string());
        }

        for attempt in 0..=max_retries {
            if attempt > 0 {
                let delay = crate::retry::retry_delay(attempt, 1000, max_delay);
                tokio::time::sleep(delay).await;
            }

            let mut output = AssistantMessage::new(
                vec![],
                "mistral-conversations".to_string(),
                "mistral".to_string(),
                model.id.clone(),
                Usage::zero(),
                StopReason::Stop,
            );

            // Emit start
            let _ = tx
                .send(StreamEvent::Start {
                    partial: crate::types::PartialAssistantMessage {
                        content: vec![],
                        api: Some("mistral-conversations".to_string()),
                        provider: Some("mistral".to_string()),
                        model: Some(model.id.clone()),
                        usage: Some(Usage::zero()),
                        stop_reason: None,
                        error_message: None,
                        timestamp: chrono::Utc::now().timestamp_millis(),
                    },
                })
                .await;

            let client = reqwest::Client::new();
            match client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        let status = resp.status();
                        let body_text = resp.text().await.unwrap_or_default();
                        let err_msg = format!("Mistral API error ({}): {}", status, body_text);
                        if crate::retry::is_retryable_http_status(status.as_u16())
                            && crate::retry::should_retry(attempt, max_retries)
                        {
                            continue;
                        }
                        output.stop_reason = StopReason::Error;
                        output.error_message = Some(err_msg);
                        let _ = tx
                            .send(StreamEvent::Error {
                                reason: StopReason::Error,
                                error: output,
                            })
                            .await;
                        return;
                    }

                    let mut parser = SseParser::new();
                    let mut text_block_idx: Option<usize> = None;
                    let mut thinking_block_idx: Option<usize> = None;
                    let mut tool_blocks: Vec<(usize, String, String, String)> = Vec::new(); // (stream_idx, id, name, partial_json)

                    use futures::StreamExt;
                    let mut chunk_stream = resp.bytes_stream();
                    while let Some(chunk_result) = chunk_stream.next().await {
                        match chunk_result {
                            Ok(chunk) => {
                                let events = parser.feed(&chunk);
                                for sse in events {
                                    process_mistral_line(
                                        &tx,
                                        &sse,
                                        &mut output,
                                        &mut text_block_idx,
                                        &mut thinking_block_idx,
                                        &mut tool_blocks,
                                    )
                                    .await;
                                }
                            }
                            Err(e) => {
                                output.stop_reason = StopReason::Error;
                                output.error_message = Some(format!("Stream error: {}", e));
                                let _ = tx
                                    .send(StreamEvent::Error {
                                        reason: StopReason::Error,
                                        error: output,
                                    })
                                    .await;
                                return;
                            }
                        }
                    }

                    let _ = tx
                        .send(StreamEvent::Done {
                            reason: output.stop_reason.clone(),
                            message: output,
                        })
                        .await;
                    return;
                }
                Err(e) => {
                    if crate::retry::is_retryable_request_error(&e)
                        && crate::retry::should_retry(attempt, max_retries)
                    {
                        continue;
                    }
                    output.stop_reason = StopReason::Error;
                    output.error_message = Some(format!("Request failed: {}", e));
                    let _ = tx
                        .send(StreamEvent::Error {
                            reason: StopReason::Error,
                            error: output,
                        })
                        .await;
                    return;
                }
            }
        }

        // Exhausted all retries
        let mut msg = AssistantMessage::new(
            vec![],
            "mistral-conversations".to_string(),
            "mistral".to_string(),
            model.id.clone(),
            Usage::zero(),
            StopReason::Error,
        );
        msg.error_message = Some("Max retries exceeded".to_string());
        let _ = tx
            .send(StreamEvent::Error {
                reason: StopReason::Error,
                error: msg,
            })
            .await;
    });

    rx
}

async fn process_mistral_line(
    tx: &tokio::sync::mpsc::Sender<StreamEvent>,
    sse: &crate::sse::SseEvent,
    output: &mut AssistantMessage,
    text_block_idx: &mut Option<usize>,
    thinking_block_idx: &mut Option<usize>,
    tool_blocks: &mut Vec<(usize, String, String, String)>,
) {
    if sse.data.trim() == "[DONE]" {
        return;
    }

    let chunk: serde_json::Value = match serde_json::from_str(&sse.data) {
        Ok(v) => v,
        Err(_) => return,
    };

    // Capture response id and model
    if output.response_id.is_none() {
        output.response_id = chunk
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
    }
    if output.response_model.is_none()
        && let Some(model_str) = chunk.get("model").and_then(|v| v.as_str())
        && !model_str.is_empty()
        && model_str != output.model
    {
        output.response_model = Some(model_str.to_string());
    }

    // Parse usage if present
    if let Some(usage) = chunk.get("usage") {
        if let Some(val) = usage.get("prompt_tokens").and_then(|v| v.as_u64()) {
            output.usage.input = val;
        }
        if let Some(val) = usage.get("completion_tokens").and_then(|v| v.as_u64()) {
            output.usage.output = val;
        }
        output.usage.total_tokens = output.usage.input + output.usage.output;
    }

    // Process first choice
    let choices = chunk.get("choices").and_then(|v| v.as_array());
    let choice = match choices.and_then(|arr| arr.first()) {
        Some(c) => c,
        None => return,
    };

    // Check finish reason
    if let Some(finish) = choice.get("finish_reason").and_then(|v| v.as_str())
        && !finish.is_empty()
        && finish != "null"
    {
        output.stop_reason = map_mistral_finish_reason(finish);
    }

    // Process delta content
    let delta = match choice.get("delta") {
        Some(d) => d,
        None => return,
    };

    // Text content
    if let Some(content) = delta.get("content").and_then(|v| v.as_str())
        && !content.is_empty()
    {
        let idx = if let Some(idx) = text_block_idx {
            *idx
        } else {
            let idx = output.content.len();
            output.content.push(ContentBlock::text(""));
            *text_block_idx = Some(idx);
            let _ = tx
                .send(StreamEvent::TextStart {
                    content_index: idx,
                    partial: partial_from_output(output),
                })
                .await;
            idx
        };
        if let ContentBlock::Text(ref mut tc) = output.content[idx] {
            tc.text.push_str(content);
        }
        let _ = tx
            .send(StreamEvent::TextDelta {
                content_index: idx,
                delta: content.to_string(),
                partial: partial_from_output(output),
            })
            .await;
    }

    // Thinking/reasoning content (OpenAI-compatible reasoning_content field)
    for thinking_field in &["reasoning_content", "reasoning", "reasoning_text"] {
        if let Some(reasoning) = delta.get(*thinking_field).and_then(|v| v.as_str())
            && !reasoning.is_empty()
        {
            let idx = if let Some(idx) = thinking_block_idx {
                *idx
            } else {
                let idx = output.content.len();
                output
                    .content
                    .push(ContentBlock::Thinking(crate::types::ThinkingContent {
                        thinking: String::new(),
                        thinking_signature: None,
                        redacted: false,
                    }));
                *thinking_block_idx = Some(idx);
                let _ = tx
                    .send(StreamEvent::ThinkingStart {
                        content_index: idx,
                        partial: partial_from_output(output),
                    })
                    .await;
                idx
            };
            if let ContentBlock::Thinking(ref mut tc) = output.content[idx] {
                tc.thinking.push_str(reasoning);
            }
            let _ = tx
                .send(StreamEvent::ThinkingDelta {
                    content_index: idx,
                    delta: reasoning.to_string(),
                    partial: partial_from_output(output),
                })
                .await;
        }
    }

    // Tool calls
    if let Some(tool_calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
        for tc_val in tool_calls {
            process_mistral_tool_call(tx, tc_val, output, tool_blocks).await;
        }
    }
}

async fn process_mistral_tool_call(
    tx: &tokio::sync::mpsc::Sender<StreamEvent>,
    tc_val: &serde_json::Value,
    output: &mut AssistantMessage,
    tool_blocks: &mut Vec<(usize, String, String, String)>,
) {
    let stream_idx = tc_val
        .get("index")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(0);
    let tc_id = tc_val.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let tc_name = tc_val
        .get("function")
        .and_then(|f| f.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Find existing block by stream index
    let existing_pos = tool_blocks.iter().position(|b| b.0 == stream_idx);

    if let Some(pos) = existing_pos {
        // Update existing block - append arguments delta
        if let Some(args) = tc_val
            .get("function")
            .and_then(|f| f.get("arguments"))
            .and_then(|v| v.as_str())
        {
            let (_idx, _id, _name, partial_json) = &mut tool_blocks[pos];
            partial_json.push_str(args);
            let arguments: serde_json::Value =
                serde_json::from_str(partial_json).unwrap_or(serde_json::Value::Null);

            // Find matching content block
            let content_idx = output
                .content
                .iter()
                .position(|c| matches!(c, ContentBlock::ToolCall(_)));
            if let Some(idx) = content_idx {
                if let ContentBlock::ToolCall(ref mut tc) = output.content[idx] {
                    tc.arguments = arguments;
                }
                let _ = tx
                    .send(StreamEvent::ToolCallDelta {
                        content_index: idx,
                        delta: args.to_string(),
                        partial: partial_from_output(output),
                    })
                    .await;
            }
        }
    } else {
        // Create new tool call block
        let id = tc_id.to_string();
        let name = tc_name.to_string();

        tool_blocks.push((stream_idx, id.clone(), name.clone(), String::new()));

        let content_idx = output.content.len();
        output
            .content
            .push(ContentBlock::tool_call(id, name, serde_json::Value::Null));

        let _ = tx
            .send(StreamEvent::ToolCallStart {
                content_index: content_idx,
                partial: partial_from_output(output),
            })
            .await;

        // Handle initial arguments if present
        if let Some(args) = tc_val
            .get("function")
            .and_then(|f| f.get("arguments"))
            .and_then(|v| v.as_str())
            && !args.is_empty()
        {
            let pos = tool_blocks.len() - 1;
            tool_blocks[pos].3.push_str(args);
            let arguments: serde_json::Value =
                serde_json::from_str(&tool_blocks[pos].3).unwrap_or(serde_json::Value::Null);
            if let ContentBlock::ToolCall(ref mut tc) = output.content[content_idx] {
                tc.arguments = arguments;
            }
            let _ = tx
                .send(StreamEvent::ToolCallDelta {
                    content_index: content_idx,
                    delta: args.to_string(),
                    partial: partial_from_output(output),
                })
                .await;
        }
    }
}

fn map_mistral_finish_reason(reason: &str) -> StopReason {
    match reason {
        "stop" => StopReason::Stop,
        "length" | "model_length" => StopReason::Length,
        "tool_calls" => StopReason::ToolUse,
        _ => StopReason::Stop,
    }
}

fn partial_from_output(output: &AssistantMessage) -> crate::types::PartialAssistantMessage {
    crate::types::PartialAssistantMessage {
        content: output.content.clone(),
        api: Some(output.api.clone()),
        provider: Some(output.provider.clone()),
        model: Some(output.model.clone()),
        usage: Some(output.usage.clone()),
        stop_reason: Some(format!("{:?}", output.stop_reason)),
        error_message: output.error_message.clone(),
        timestamp: chrono::Utc::now().timestamp_millis(),
    }
}

/// Simple streaming version
pub fn stream_simple_mistral(
    model: Model,
    context: Context,
    options: Option<crate::types::SimpleStreamOptions>,
) -> tokio::sync::mpsc::Receiver<StreamEvent> {
    let stream_opts = options.map(|o| {
        let mut opts = o.base;
        if o.reasoning.is_some() {
            opts.reasoning = o.reasoning;
        }
        opts
    });
    stream_mistral(model, context, stream_opts)
}
