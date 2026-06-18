//! Azure OpenAI Responses API provider with SSE streaming
//! Uses Azure's OpenAI-compatible Responses API with api-key header

use crate::types::{
    AssistantMessage, ContentBlock, Context, Model, StopReason, StreamEvent, StreamOptions, Usage,
};

/// Stream a response from Azure OpenAI's Responses API
pub fn stream_azure_openai_responses(
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
        let api_key = std::env::var("AZURE_OPENAI_API_KEY").unwrap_or_default();
        if api_key.is_empty() {
            let mut msg = AssistantMessage::new(
                vec![],
                "azure-openai-responses".to_string(),
                "azure-openai-responses".to_string(),
                model.id.clone(),
                Usage::zero(),
                StopReason::Error,
            );
            msg.error_message = Some("AZURE_OPENAI_API_KEY is not set.".to_string());
            let _ = tx
                .send(StreamEvent::Error {
                    reason: StopReason::Error,
                    error: msg,
                })
                .await;
            return;
        }

        let url = format!("{}/v1/responses", model.base_url.trim_end_matches('/'));

        // Build input items (same as OpenAI Responses)
        let mut input_items: Vec<serde_json::Value> = Vec::new();
        for msg in &context.messages {
            match msg {
                crate::types::Message::User(u) => {
                    let mut content_parts: Vec<serde_json::Value> = Vec::new();
                    for block in &u.content {
                        match block {
                            ContentBlock::Text(t) => {
                                content_parts.push(serde_json::json!({
                                    "type": "input_text",
                                    "text": t.text,
                                }));
                            }
                            ContentBlock::Image(img) => {
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
                            ContentBlock::Text(t) => {
                                content_parts.push(serde_json::json!({
                                    "type": "output_text",
                                    "text": t.text,
                                }));
                            }
                            ContentBlock::ToolCall(tc) => {
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
                            if let ContentBlock::Text(t) = c {
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
                "azure-openai-responses".to_string(),
                "azure-openai-responses".to_string(),
                model.id.clone(),
                Usage::zero(),
                StopReason::Stop,
            );

            // Emit start
            let _ = tx
                .send(StreamEvent::Start {
                    partial: crate::types::PartialAssistantMessage {
                        content: vec![],
                        api: Some("azure-openai-responses".to_string()),
                        provider: Some("azure-openai-responses".to_string()),
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
                .header("api-key", &api_key)
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        let status = resp.status();
                        let body_text = resp.text().await.unwrap_or_default();
                        let err_msg = format!("Azure OpenAI API error ({}): {}", status, body_text);
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
                    let mut response_id: Option<String> = None;

                    use futures::StreamExt;
                    let mut chunk_stream = resp.bytes_stream();
                    while let Some(chunk_result) = chunk_stream.next().await {
                        match chunk_result {
                            Ok(chunk) => {
                                let events = parser.feed(&chunk);
                                for sse in events {
                                    process_azure_event(
                                        &tx,
                                        &sse,
                                        &mut output,
                                        &mut text_block_idx,
                                        &mut response_id,
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
            "azure-openai-responses".to_string(),
            "azure-openai-responses".to_string(),
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

async fn process_azure_event(
    tx: &tokio::sync::mpsc::Sender<StreamEvent>,
    sse: &crate::sse::SseEvent,
    output: &mut AssistantMessage,
    text_block_idx: &mut Option<usize>,
    _response_id: &mut Option<String>,
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
            output.content.push(ContentBlock::text(""));
            *text_block_idx = Some(idx);
            let _ = tx
                .send(StreamEvent::TextStart {
                    content_index: idx,
                    partial: partial_from_output(output),
                })
                .await;
        }
        "response.output_text.delta" => {
            if let Some(delta) = data.get("delta").and_then(|v| v.as_str()) {
                if let Some(idx) = *text_block_idx {
                    if idx < output.content.len() {
                        if let ContentBlock::Text(ref mut tc) = output.content[idx] {
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
pub fn stream_simple_azure_openai_responses(
    model: Model,
    context: Context,
    _options: Option<crate::types::SimpleStreamOptions>,
) -> tokio::sync::mpsc::Receiver<StreamEvent> {
    stream_azure_openai_responses(model, context, None)
}
