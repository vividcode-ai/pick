//! Google Generative AI (Gemini) provider with SSE streaming
//! Uses the Google Gemini streamGenerateContent REST API

use crate::sse::SseParser;
use crate::types::{
    AssistantMessage, ContentBlock, Context, Model, StopReason, StreamEvent, StreamOptions, Usage,
};

/// Stream a response from Google's Gemini API
pub fn stream_google(
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
        let api_key = std::env::var("GOOGLE_API_KEY").unwrap_or_default();

        if api_key.is_empty() {
            let mut msg = AssistantMessage::new(
                vec![],
                "google-generative-ai".to_string(),
                "google".to_string(),
                model.id.clone(),
                Usage::zero(),
                StopReason::Error,
            );
            msg.error_message = Some("GOOGLE_API_KEY is not set.".to_string());
            let _ = tx
                .send(StreamEvent::Error {
                    reason: StopReason::Error,
                    error: msg,
                })
                .await;
            return;
        }

        let base_url = model.base_url.trim_end_matches('/');
        let url = format!(
            "{}/v1beta/models/{}:streamGenerateContent?alt=sse",
            base_url, model.id
        );

        // Build request body using Google's format
        let mut contents: Vec<serde_json::Value> = Vec::new();

        for msg in &context.messages {
            match msg {
                crate::types::Message::User(u) => {
                    let mut parts = Vec::new();
                    for block in &u.content {
                        match block {
                            ContentBlock::Text(t) => {
                                parts.push(serde_json::json!({"text": t.text}));
                            }
                            ContentBlock::Image(img) => {
                                parts.push(serde_json::json!({
                                    "inlineData": {
                                        "mimeType": img.mime_type,
                                        "data": img.data,
                                    }
                                }));
                            }
                            _ => {}
                        }
                    }
                    if !parts.is_empty() {
                        contents.push(serde_json::json!({
                            "role": "user",
                            "parts": parts,
                        }));
                    }
                }
                crate::types::Message::Assistant(a) => {
                    let mut parts = Vec::new();
                    for block in &a.content {
                        match block {
                            ContentBlock::Text(t)
                                if !t.text.trim().is_empty() => {
                                    parts.push(serde_json::json!({"text": t.text}));
                                }
                            ContentBlock::Thinking(th)
                                if !th.thinking.trim().is_empty() => {
                                    parts.push(serde_json::json!({
                                        "thought": true,
                                        "text": th.thinking,
                                    }));
                                }
                            ContentBlock::ToolCall(tc) => {
                                parts.push(serde_json::json!({
                                    "functionCall": {
                                        "name": tc.name,
                                        "args": tc.arguments,
                                    }
                                }));
                            }
                            _ => {}
                        }
                    }
                    if !parts.is_empty() {
                        contents.push(serde_json::json!({
                            "role": "model",
                            "parts": parts,
                        }));
                    }
                }
                crate::types::Message::ToolResult(tr) => {
                    let mut parts = Vec::new();
                    let text_content: Vec<String> = tr
                        .content
                        .iter()
                        .filter_map(|c| {
                            if let ContentBlock::Text(t) = c {
                                Some(t.text.clone())
                            } else {
                                None
                            }
                        })
                        .collect();
                    let text = text_content.join("\n");

                    parts.push(serde_json::json!({
                        "functionResponse": {
                            "name": tr.tool_name,
                            "response": if tr.is_error {
                                serde_json::json!({"error": text})
                            } else {
                                serde_json::json!({"output": text})
                            },
                        }
                    }));

                    contents.push(serde_json::json!({
                        "role": "user",
                        "parts": parts,
                    }));
                }
            }
        }

        let mut body = serde_json::json!({
            "contents": contents,
        });

        if let Some(system) = &context.system_prompt {
            body["systemInstruction"] = serde_json::json!({"parts": [{"text": system}]});
        }
        if let Some(tools) = &context.tools {
            let function_declarations: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    })
                })
                .collect();
            body["tools"] = serde_json::json!([{
                "functionDeclarations": function_declarations,
            }]);
        }

        for attempt in 0..=max_retries {
            if attempt > 0 {
                tokio::time::sleep(crate::retry::retry_delay(attempt, 1000, max_delay)).await;
            }

            let mut output = AssistantMessage::new(
                vec![],
                "google-generative-ai".to_string(),
                "google".to_string(),
                model.id.clone(),
                Usage::zero(),
                StopReason::Stop,
            );

            let _ = tx
                .send(StreamEvent::Start {
                    partial: crate::types::PartialAssistantMessage {
                        content: vec![],
                        api: Some("google-generative-ai".to_string()),
                        provider: Some("google".to_string()),
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
                .header("x-goog-api-key", &api_key)
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    if !status.is_success() {
                        if crate::retry::is_retryable_http_status(status.as_u16())
                            && crate::retry::should_retry(attempt, max_retries)
                        {
                            continue;
                        }
                        let body_text = resp.text().await.unwrap_or_default();
                        let err_msg = format!("Google AI API error ({}): {}", status, body_text);
                        output.stop_reason = StopReason::Error;
                        output.error_message = Some(err_msg);
                        let _ = tx
                            .send(StreamEvent::Error {
                                reason: StopReason::Error,
                                error: output.clone(),
                            })
                            .await;
                        return;
                    }

                    let mut parser = SseParser::new();
                    let mut text_block_idx: Option<usize> = None;
                    let mut thinking_block_idx: Option<usize> = None;

                    use futures::StreamExt;
                    let mut chunk_stream = resp.bytes_stream();
                    while let Some(chunk_result) = chunk_stream.next().await {
                        match chunk_result {
                            Ok(chunk) => {
                                let events = parser.feed(&chunk);
                                for sse in events {
                                    process_google_chunk(
                                        &tx,
                                        &sse,
                                        &mut output,
                                        &mut text_block_idx,
                                        &mut thinking_block_idx,
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
                                        error: output.clone(),
                                    })
                                    .await;
                                return;
                            }
                        }
                    }

                    // Send done
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
                            error: output.clone(),
                        })
                        .await;
                    return;
                }
            }
        }

        let mut msg = AssistantMessage::new(
            vec![],
            "google-generative-ai".to_string(),
            "google".to_string(),
            model.id.clone(),
            Usage::zero(),
            StopReason::Error,
        );
        msg.error_message = Some("Max retries exceeded.".to_string());
        let _ = tx
            .send(StreamEvent::Error {
                reason: StopReason::Error,
                error: msg,
            })
            .await;
    });

    rx
}

async fn process_google_chunk(
    tx: &tokio::sync::mpsc::Sender<StreamEvent>,
    sse: &crate::sse::SseEvent,
    output: &mut AssistantMessage,
    text_block_idx: &mut Option<usize>,
    thinking_block_idx: &mut Option<usize>,
) {
    // Google returns JSON array: [{"candidates": [...], ...}]
    let data: serde_json::Value = match serde_json::from_str(&sse.data) {
        Ok(v) => v,
        Err(_) => return,
    };

    let items = match data.as_array() {
        Some(arr) => arr,
        None => {
            // Single object
            process_google_response(&data, tx, output, text_block_idx, thinking_block_idx).await;
            return;
        }
    };

    for item in items {
        process_google_response(item, tx, output, text_block_idx, thinking_block_idx).await;
    }
}

async fn process_google_response(
    data: &serde_json::Value,
    tx: &tokio::sync::mpsc::Sender<StreamEvent>,
    output: &mut AssistantMessage,
    text_block_idx: &mut Option<usize>,
    thinking_block_idx: &mut Option<usize>,
) {
    // Capture response ID
    if output.response_id.is_none()
        && let Some(id) = data.get("responseId").and_then(|v| v.as_str())
            && !id.is_empty() {
                output.response_id = Some(id.to_string());
            }

    // Process usage metadata
    if let Some(usage) = data.get("usageMetadata") {
        let input = usage
            .get("promptTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let output_tokens = usage
            .get("candidatesTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let thoughts = usage
            .get("thoughtsTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cache_read = usage
            .get("cachedContentTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let total = usage
            .get("totalTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(input + output_tokens + thoughts);

        output.usage.input = input;
        output.usage.output = output_tokens + thoughts;
        output.usage.cache_read = cache_read;
        output.usage.total_tokens = total;
    }

    // Process candidates
    let candidates = match data.get("candidates").and_then(|v| v.as_array()) {
        Some(c) => c,
        None => return,
    };

    for candidate in candidates {
        // Check finish reason
        if let Some(reason) = candidate.get("finishReason").and_then(|v| v.as_str()) {
            output.stop_reason = match reason {
                "STOP" => StopReason::Stop,
                "MAX_TOKENS" => StopReason::Length,
                "SAFETY" | "BLOCKLIST" | "PROHIBITED_CONTENT" => StopReason::Error,
                _ => StopReason::Stop,
            };
            // If tool calls are present, override to ToolUse
            if !output.content.is_empty()
                && output
                    .content
                    .iter()
                    .any(|c| matches!(c, ContentBlock::ToolCall(_)))
            {
                output.stop_reason = StopReason::ToolUse;
            }
        }

        let content = match candidate.get("content") {
            Some(c) => c,
            None => continue,
        };

        let parts = match content.get("parts").and_then(|v| v.as_array()) {
            Some(p) => p,
            None => continue,
        };

        for part in parts {
            if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                if text.is_empty() {
                    continue;
                }

                let is_thinking = part
                    .get("thought")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if is_thinking {
                    let idx = if let Some(idx) = thinking_block_idx {
                        *idx
                    } else {
                        let idx = output.content.len();
                        output.content.push(ContentBlock::Thinking(
                            crate::types::ThinkingContent {
                                thinking: String::new(),
                                thinking_signature: None,
                                redacted: false,
                            },
                        ));
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
                        tc.thinking.push_str(text);
                        // Preserve thought signature
                        if let Some(sig) = part.get("thoughtSignature").and_then(|v| v.as_str())
                            && !sig.is_empty() {
                                tc.thinking_signature = Some(sig.to_string());
                            }
                    }
                    let _ = tx
                        .send(StreamEvent::ThinkingDelta {
                            content_index: idx,
                            delta: text.to_string(),
                            partial: partial_from_output(output),
                        })
                        .await;
                } else {
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
                        tc.text.push_str(text);
                        if let Some(sig) = part.get("thoughtSignature").and_then(|v| v.as_str())
                            && !sig.is_empty() {
                                tc.text_signature = Some(sig.to_string());
                            }
                    }
                    let _ = tx
                        .send(StreamEvent::TextDelta {
                            content_index: idx,
                            delta: text.to_string(),
                            partial: partial_from_output(output),
                        })
                        .await;
                }
            }

            // Handle function calls (tool use)
            if let Some(fc) = part.get("functionCall") {
                let name = fc.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = fc.get("args").unwrap_or(&serde_json::Value::Null);

                let idx = output.content.len();
                output.content.push(ContentBlock::tool_call(
                    format!("{}_{}", name, chrono::Utc::now().timestamp_millis()),
                    name,
                    args.clone(),
                ));

                let _ = tx
                    .send(StreamEvent::ToolCallStart {
                        content_index: idx,
                        partial: partial_from_output(output),
                    })
                    .await;

                let _ = tx
                    .send(StreamEvent::ToolCallDelta {
                        content_index: idx,
                        delta: serde_json::to_string(args).unwrap_or_default(),
                        partial: partial_from_output(output),
                    })
                    .await;

                // End the tool call immediately since Google sends it all at once
                if let ContentBlock::ToolCall(tc) = &output.content[idx] {
                    let _ = tx
                        .send(StreamEvent::ToolCallEnd {
                            content_index: idx,
                            tool_call: tc.clone(),
                            partial: partial_from_output(output),
                        })
                        .await;
                }
            }
        }
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
pub fn stream_simple_google(
    model: Model,
    context: Context,
    _options: Option<crate::types::SimpleStreamOptions>,
) -> tokio::sync::mpsc::Receiver<StreamEvent> {
    stream_google(model, context, None)
}
