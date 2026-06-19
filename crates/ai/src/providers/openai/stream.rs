use crate::providers::cloudflare;
use crate::sse::SseParser;
use crate::types::{
    AssistantMessage, ContentBlock, Context, Model, StopReason, StreamEvent, StreamOptions, Usage,
};

/// Streaming state for tool call blocks
struct StreamingBlock {
    tool_index: Option<usize>,
    tool_call_id: Option<String>,
    content_index: usize,
    partial_json: String,
    arguments: serde_json::Value,
}

/// Stream a response from OpenAI's Chat Completions API
pub fn stream_openai_completions(
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
        let api_key = crate::utils::env_api_keys::get_env_api_key(model.provider.as_str())
            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
            .unwrap_or_default();

        if api_key.is_empty() {
            let mut msg = AssistantMessage::new(
                vec![],
                "openai-completions".to_string(),
                "openai".to_string(),
                model.id.clone(),
                Usage::zero(),
                StopReason::Error,
            );
            msg.error_message = Some(format!(
                "{}_API_KEY is not set.",
                model.provider.as_str().to_uppercase().replace('-', "_")
            ));
            let _ = tx
                .send(StreamEvent::Error {
                    reason: StopReason::Error,
                    error: msg,
                })
                .await;
            return;
        }

        let base_url = if cloudflare::is_cloudflare_provider(&model.provider) {
            match cloudflare::resolve_cloudflare_base_url(&model.base_url) {
                Ok(url) => url,
                Err(_) => {
                    let msg = AssistantMessage::new(
                        vec![],
                        "openai-completions".to_string(),
                        "openai".to_string(),
                        model.id.clone(),
                        Usage::zero(),
                        StopReason::Error,
                    );
                    let _ = tx
                        .send(StreamEvent::Error {
                            reason: StopReason::Error,
                            error: msg,
                        })
                        .await;
                    return;
                }
            }
        } else {
            model.base_url.clone()
        };
        let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));

        let messages: Vec<serde_json::Value> = super::convert_to_openai_messages(&context.messages);

        let mut body = serde_json::json!({
            "model": model.id,
            "messages": messages,
            "stream": true,
        });
        if let Some(tools) = &context.tools {
            let tools_json: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(tools_json);
        }

        let is_cloudflare_gateway = cloudflare::is_cloudflare_ai_gateway(&model.provider);

        for attempt in 0..=max_retries {
            if attempt > 0 {
                let delay = crate::retry::retry_delay(attempt, 1000, max_delay);
                tokio::time::sleep(delay).await;
            }

            let mut output = AssistantMessage::new(
                vec![],
                "openai-completions".to_string(),
                "openai".to_string(),
                model.id.clone(),
                Usage::zero(),
                StopReason::Stop,
            );

            let _ = tx
                .send(StreamEvent::Start {
                    partial: crate::types::PartialAssistantMessage {
                        content: vec![],
                        api: Some("openai-completions".to_string()),
                        provider: Some("openai".to_string()),
                        model: Some(model.id.clone()),
                        usage: Some(Usage::zero()),
                        stop_reason: None,
                        error_message: None,
                        timestamp: chrono::Utc::now().timestamp_millis(),
                    },
                })
                .await;

            let client = reqwest::Client::new();
            let request_builder = client.post(&url).header("content-type", "application/json");
            let request_builder = if is_cloudflare_gateway {
                request_builder.header("cf-aig-authorization", format!("Bearer {}", api_key))
            } else {
                request_builder.header("Authorization", format!("Bearer {}", api_key))
            };
            match request_builder.json(&body).send().await {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        let status = resp.status();
                        let body_text = resp.text().await.unwrap_or_default();
                        let err_msg = format!("OpenAI API error ({}): {}", status, body_text);
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
                    let mut tool_blocks_by_idx: Vec<StreamingBlock> = Vec::new();
                    let mut has_finish_reason = false;

                    use futures::StreamExt;
                    let mut chunk_stream = resp.bytes_stream();
                    while let Some(chunk_result) = chunk_stream.next().await {
                        match chunk_result {
                            Ok(chunk) => {
                                let events = parser.feed(&chunk);
                                for sse in events {
                                    process_openai_line(
                                        &tx,
                                        &sse,
                                        &mut output,
                                        &mut text_block_idx,
                                        &mut thinking_block_idx,
                                        &mut tool_blocks_by_idx,
                                        &mut has_finish_reason,
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

        let mut msg = AssistantMessage::new(
            vec![],
            "openai-completions".to_string(),
            "openai".to_string(),
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

async fn process_openai_line(
    tx: &tokio::sync::mpsc::Sender<StreamEvent>,
    sse: &crate::sse::SseEvent,
    output: &mut AssistantMessage,
    text_block_idx: &mut Option<usize>,
    thinking_block_idx: &mut Option<usize>,
    tool_blocks_by_idx: &mut Vec<StreamingBlock>,
    has_finish_reason: &mut bool,
) {
    if sse.data.trim() == "[DONE]" {
        return;
    }

    let chunk: serde_json::Value = match serde_json::from_str(&sse.data) {
        Ok(v) => v,
        Err(_) => return,
    };

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

    if let Some(usage) = chunk.get("usage") {
        parse_chunk_usage(output, usage);
    }

    let choices = chunk.get("choices").and_then(|v| v.as_array());
    let choice = choices.and_then(|arr| arr.first());
    let choice = match choice {
        Some(c) => c,
        None => return,
    };

    if let Some(finish) = choice.get("finish_reason").and_then(|v| v.as_str()) {
        *has_finish_reason = true;
        if !finish.is_empty() && finish != "null" {
            output.stop_reason = map_openai_finish_reason(finish);
        }
    }

    if output.usage.input == 0
        && output.usage.output == 0
        && let Some(usage) = choice.get("usage")
    {
        parse_chunk_usage(output, usage);
    }

    let delta = match choice.get("delta") {
        Some(d) => d,
        None => return,
    };

    if let Some(content) = delta.get("content").and_then(|v| v.as_str())
        && !content.is_empty()
    {
        let idx = ensure_text_block(output, text_block_idx, tx).await;
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

    for thinking_field in &["reasoning_content", "reasoning", "reasoning_text"] {
        if let Some(reasoning) = delta.get(*thinking_field).and_then(|v| v.as_str())
            && !reasoning.is_empty()
        {
            let idx = ensure_thinking_block(output, thinking_block_idx, tx, None).await;
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

    if let Some(details) = delta.get("reasoning_details").and_then(|v| v.as_array()) {
        for detail in details {
            if let Some(sig) = detail.get("signature").and_then(|v| v.as_str()) {
                let idx = ensure_thinking_block(output, thinking_block_idx, tx, None).await;
                if let Some(ref mut tc) = output.content.get_mut(idx).and_then(|c| {
                    if let ContentBlock::Thinking(tc) = c {
                        Some(tc)
                    } else {
                        None
                    }
                }) {
                    tc.thinking_signature =
                        Some(tc.thinking_signature.clone().unwrap_or_default() + sig);
                }
            }
        }
    }

    if let Some(tool_calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
        for tc_val in tool_calls {
            process_tool_call_delta(tx, tc_val, output, tool_blocks_by_idx).await;
        }
    }
}

async fn ensure_text_block(
    output: &mut AssistantMessage,
    text_block_idx: &mut Option<usize>,
    tx: &tokio::sync::mpsc::Sender<StreamEvent>,
) -> usize {
    if let Some(idx) = text_block_idx {
        return *idx;
    }

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
}

async fn ensure_thinking_block(
    output: &mut AssistantMessage,
    thinking_block_idx: &mut Option<usize>,
    tx: &tokio::sync::mpsc::Sender<StreamEvent>,
    signature: Option<String>,
) -> usize {
    if let Some(idx) = thinking_block_idx {
        return *idx;
    }

    let idx = output.content.len();
    output
        .content
        .push(ContentBlock::Thinking(crate::types::ThinkingContent {
            thinking: String::new(),
            thinking_signature: signature,
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
}

fn extract_tool_args(field: &serde_json::Value) -> (String, serde_json::Value) {
    let raw = field.get("arguments");
    match raw {
        Some(v) if v.is_string() => {
            let s = v.as_str().unwrap_or("").to_string();
            let parsed = if s.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::from_str(&s).unwrap_or(serde_json::Value::Null)
            };
            (s, parsed)
        }
        Some(v) => (String::new(), v.clone()),
        None => (String::new(), serde_json::Value::Null),
    }
}

async fn process_tool_call_delta(
    tx: &tokio::sync::mpsc::Sender<StreamEvent>,
    tc_val: &serde_json::Value,
    output: &mut AssistantMessage,
    tool_blocks: &mut Vec<StreamingBlock>,
) {
    let stream_idx = tc_val
        .get("index")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let tc_id = tc_val.get("id").and_then(|v| v.as_str()).unwrap_or("");

    let existing_pos = tool_blocks.iter().position(|b| {
        let idx_match = match (b.tool_index, stream_idx) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        };
        idx_match || (b.tool_call_id.as_deref() == Some(tc_id) && !tc_id.is_empty())
    });

    if let Some(pos) = existing_pos {
        let func = tc_val.get("function");
        if let Some(f) = func {
            let (partial_str, parsed) = extract_tool_args(f);

            let mut updated = false;
            if !partial_str.is_empty() {
                tool_blocks[pos].partial_json += &partial_str;
                if let Ok(v) =
                    serde_json::from_str::<serde_json::Value>(&tool_blocks[pos].partial_json)
                {
                    tool_blocks[pos].arguments = v;
                    updated = true;
                }
            } else if !parsed.is_null() {
                tool_blocks[pos].arguments = parsed.clone();
                updated = true;
            }

            if updated {
                let idx = tool_blocks[pos].content_index;
                if idx < output.content.len() {
                    if let ContentBlock::ToolCall(ref mut tc) = output.content[idx] {
                        tc.arguments = tool_blocks[pos].arguments.clone();
                    }
                    let _ = tx
                        .send(StreamEvent::ToolCallDelta {
                            content_index: idx,
                            delta: partial_str,
                            partial: partial_from_output(output),
                        })
                        .await;
                }
            }
        }
    } else {
        let id = tc_id.to_string();
        let name = tc_val
            .get("function")
            .and_then(|f| f.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let (partial_str, parsed) = tc_val
            .get("function")
            .map(extract_tool_args)
            .unwrap_or_default();

        let content_idx = output.content.len();
        output
            .content
            .push(ContentBlock::tool_call(id.clone(), name, parsed.clone()));

        tool_blocks.push(StreamingBlock {
            tool_index: stream_idx,
            tool_call_id: if id.is_empty() { None } else { Some(id) },
            content_index: content_idx,
            partial_json: partial_str,
            arguments: parsed,
        });

        let _ = tx
            .send(StreamEvent::ToolCallStart {
                content_index: content_idx,
                partial: partial_from_output(output),
            })
            .await;
    }
}

fn parse_chunk_usage(output: &mut AssistantMessage, usage: &serde_json::Value) {
    if let Some(val) = usage
        .get("prompt_tokens")
        .or_else(|| usage.get("input_tokens"))
        .and_then(|v| v.as_u64())
    {
        output.usage.input = val;
    }
    if let Some(val) = usage
        .get("completion_tokens")
        .or_else(|| usage.get("output_tokens"))
        .and_then(|v| v.as_u64())
    {
        output.usage.output = val;
    }
    if let Some(val) = usage.get("total_tokens").and_then(|v| v.as_u64()) {
        output.usage.total_tokens = val;
    }
    output.usage.total_tokens = output.usage.input + output.usage.output;
}

fn map_openai_finish_reason(reason: &str) -> StopReason {
    match reason {
        "stop" => StopReason::Stop,
        "length" => StopReason::Length,
        "tool_calls" => StopReason::ToolUse,
        "content_filter" => StopReason::Error,
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
