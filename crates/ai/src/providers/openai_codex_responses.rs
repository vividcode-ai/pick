//! OpenAI Codex Responses API provider with SSE streaming
//! Uses the OpenAI Codex API with OpenAI Responses-compatible format

use crate::types::{
    AssistantMessage, Context, Model, StopReason, StreamEvent, StreamOptions, Usage,
};

/// Stream a response from OpenAI Codex Responses API
pub fn stream_openai_codex_responses(
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
            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
            .unwrap_or_default();
        if api_key.is_empty() {
            let mut msg = AssistantMessage::new(
                vec![],
                "openai-codex-responses".to_string(),
                "openai-codex".to_string(),
                model.id.clone(),
                Usage::zero(),
                StopReason::Error,
            );
            msg.error_message = Some("OPENAI_API_KEY is not set.".to_string());
            let _ = tx
                .send(StreamEvent::Error {
                    reason: StopReason::Error,
                    error: msg,
                })
                .await;
            return;
        }

        let url = format!("{}/v1/responses", model.base_url.trim_end_matches('/'));

        // Build input items (same format as OpenAI Responses)
        let mut input_items: Vec<serde_json::Value> = Vec::new();
        for msg in &context.messages {
            match msg {
                crate::types::Message::User(u) => {
                    let mut content_parts: Vec<serde_json::Value> = Vec::new();
                    for block in &u.content {
                        match block {
                            crate::types::ContentBlock::Text(t) => {
                                content_parts.push(serde_json::json!({
                                    "type": "input_text",
                                    "text": t.text,
                                }));
                            }
                            crate::types::ContentBlock::Image(img) => {
                                content_parts.push(serde_json::json!({
                                    "type": "input_image",
                                    "image_url": format!("data:{};base64,{}", img.mime_type, img.data),
                                }));
                            }
                            _ => {}
                        }
                    }
                    input_items.push(serde_json::json!({
                        "role": "user",
                        "content": content_parts,
                    }));
                }
                crate::types::Message::Assistant(a) => {
                    let mut content_parts: Vec<serde_json::Value> = Vec::new();
                    for block in &a.content {
                        match block {
                            crate::types::ContentBlock::Text(t) => {
                                content_parts.push(serde_json::json!({
                                    "type": "output_text",
                                    "text": t.text,
                                }));
                            }
                            crate::types::ContentBlock::ToolCall(tc) => {
                                content_parts.push(serde_json::json!({
                                    "type": "function_call",
                                    "id": tc.id,
                                    "name": tc.name,
                                    "arguments": serde_json::to_string(&tc.arguments).unwrap_or_default(),
                                }));
                            }
                            _ => {}
                        }
                    }
                    if !content_parts.is_empty() {
                        input_items.push(serde_json::json!({
                            "role": "assistant",
                            "content": content_parts,
                        }));
                    }
                }
                crate::types::Message::ToolResult(tr) => {
                    let text_content: Vec<String> = tr
                        .content
                        .iter()
                        .filter_map(|c| {
                            if let crate::types::ContentBlock::Text(t) = c {
                                Some(t.text.clone())
                            } else {
                                None
                            }
                        })
                        .collect();
                    input_items.push(serde_json::json!({
                        "type": "function_call_output",
                        "call_id": tr.tool_call_id,
                        "output": text_content.join("\n"),
                    }));
                }
            }
        }

        let mut body = serde_json::json!({
            "model": model.id,
            "input": input_items,
            "stream": true,
        });
        if let Some(system) = &context.system_prompt {
            body["instructions"] = serde_json::json!(system);
        }
        if let Some(tools) = &context.tools {
            let tools_json: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(tools_json);
        }

        for attempt in 0..=max_retries {
            if attempt > 0 {
                let delay = crate::retry::retry_delay(attempt, 1000, max_delay);
                tokio::time::sleep(delay).await;
            }

            let mut output = AssistantMessage::new(
                vec![],
                "openai-codex-responses".to_string(),
                "openai-codex".to_string(),
                model.id.clone(),
                Usage::zero(),
                StopReason::Stop,
            );

            // Emit start
            let _ = tx
                .send(StreamEvent::Start {
                    partial: crate::types::PartialAssistantMessage {
                        content: vec![],
                        api: Some("openai-codex-responses".to_string()),
                        provider: Some("openai-codex".to_string()),
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
                        let err_msg = format!("OpenAI Codex API error ({}): {}", status, body_text);
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

                    let mut parser = crate::sse::SseParser::new();
                    let mut text_block_idx: Option<usize> = None;
                    let mut _response_id: Option<String> = None;

                    use futures::StreamExt;
                    let mut chunk_stream = resp.bytes_stream();
                    while let Some(chunk_result) = chunk_stream.next().await {
                        match chunk_result {
                            Ok(chunk) => {
                                let events = parser.feed(&chunk);
                                for sse in events {
                                    process_codex_event(
                                        &tx,
                                        &sse,
                                        &mut output,
                                        &mut text_block_idx,
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
            "openai-codex-responses".to_string(),
            "openai-codex".to_string(),
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

#[allow(unused_variables)]
async fn process_codex_event(
    tx: &tokio::sync::mpsc::Sender<StreamEvent>,
    sse: &crate::sse::SseEvent,
    output: &mut AssistantMessage,
    text_block_idx: &mut Option<usize>,
) {
    let data: serde_json::Value = match serde_json::from_str(&sse.data) {
        Ok(v) => v,
        Err(_) => return,
    };

    let event_type = match sse.event.as_deref() {
        Some(t) => t,
        None => return,
    };

    match event_type {
        "response.created" => {
            if let Some(id) = data
                .get("response")
                .and_then(|r| r.get("id"))
                .and_then(|v| v.as_str())
            {
                output.response_id = Some(id.to_string());
            }
        }
        "response.output_text.added" => {
            let idx = output.content.len();
            output.content.push(crate::types::ContentBlock::text(""));
            *text_block_idx = Some(idx);
            let _ = tx
                .send(StreamEvent::TextStart {
                    content_index: idx,
                    partial: partial_from_output(output),
                })
                .await;
        }
        "response.output_text.delta" => {
            if let Some(delta) = data.get("delta").and_then(|v| v.as_str())
                && let Some(idx) = *text_block_idx
                && idx < output.content.len()
            {
                if let crate::types::ContentBlock::Text(ref mut tc) = output.content[idx] {
                    tc.text.push_str(delta);
                }
                let _ = tx
                    .send(StreamEvent::TextDelta {
                        content_index: idx,
                        delta: delta.to_string(),
                        partial: partial_from_output(output),
                    })
                    .await;
            }
        }
        "response.completed" => {
            if let Some(response) = data.get("response") {
                if let Some(usage) = response.get("usage") {
                    if let Some(val) = usage.get("input_tokens").and_then(|v| v.as_u64()) {
                        output.usage.input = val;
                    }
                    if let Some(val) = usage.get("output_tokens").and_then(|v| v.as_u64()) {
                        output.usage.output = val;
                    }
                    output.usage.total_tokens = output.usage.input + output.usage.output;
                }
                if let Some(status) = response.get("status").and_then(|v| v.as_str()) {
                    output.stop_reason = match status {
                        "completed" => StopReason::Stop,
                        "incomplete" => StopReason::Length,
                        "failed" => StopReason::Error,
                        _ => StopReason::Stop,
                    };
                }
            }
        }
        "response.failed" => {
            output.stop_reason = StopReason::Error;
            if let Some(error) = data.get("error") {
                output.error_message = Some(
                    error
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown error")
                        .to_string(),
                );
            }
        }
        _ => {}
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
pub fn stream_simple_openai_codex_responses(
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
    stream_openai_codex_responses(model, context, stream_opts)
}
