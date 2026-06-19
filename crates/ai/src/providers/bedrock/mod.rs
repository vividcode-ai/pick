//! AWS Bedrock Converse Stream API provider with SigV4 signing

pub(crate) mod events;
pub(crate) mod signing;

use crate::types::{
    AssistantMessage, ContentBlock, Context, Model, StopReason, StreamEvent, StreamOptions, Usage,
};

fn build_bedrock_request(model: &Model, context: &Context, region: &str) -> (String, Vec<u8>) {
    let base_url = if model.base_url.contains("amazonaws.com") {
        model.base_url.trim_end_matches('/').to_string()
    } else {
        format!("https://bedrock-runtime.{}.amazonaws.com", region)
    };

    let url = format!("{}/model/{}", base_url, model.id);
    let stream_url = format!("{}/converse-stream", url);

    let mut bedrock_messages: Vec<serde_json::Value> = Vec::new();
    for msg in &context.messages {
        match msg {
            crate::types::Message::User(u) => {
                let mut content_blocks: Vec<serde_json::Value> = Vec::new();
                for block in &u.content {
                    match block {
                        ContentBlock::Text(t) => {
                            content_blocks.push(serde_json::json!({"text": t.text}));
                        }
                        ContentBlock::Image(img) => {
                            let format = match img.mime_type.as_str() {
                                "image/jpeg" | "image/jpg" => "jpeg",
                                "image/png" => "png",
                                "image/gif" => "gif",
                                "image/webp" => "webp",
                                _ => "png",
                            };
                            content_blocks.push(serde_json::json!({
                                "image": {
                                    "format": format,
                                    "source": {"bytes": img.data},
                                }
                            }));
                        }
                        _ => {}
                    }
                }
                if !content_blocks.is_empty() {
                    bedrock_messages.push(serde_json::json!({
                        "role": "user",
                        "content": content_blocks,
                    }));
                }
            }
            crate::types::Message::Assistant(a) => {
                let mut content_blocks: Vec<serde_json::Value> = Vec::new();
                for block in &a.content {
                    match block {
                        ContentBlock::Text(t)
                            if !t.text.trim().is_empty() => {
                                content_blocks.push(serde_json::json!({"text": t.text}));
                            }
                        ContentBlock::Thinking(th)
                            if !th.thinking.trim().is_empty() => {
                                if let Some(sig) = &th.thinking_signature {
                                    content_blocks.push(serde_json::json!({
                                        "reasoningContent": {
                                            "reasoningText": {
                                                "text": th.thinking,
                                                "signature": sig,
                                            }
                                        }
                                    }));
                                } else {
                                    content_blocks.push(serde_json::json!({
                                        "reasoningContent": {
                                            "reasoningText": {
                                                "text": th.thinking,
                                            }
                                        }
                                    }));
                                }
                            }
                        ContentBlock::ToolCall(tc) => {
                            content_blocks.push(serde_json::json!({
                                "toolUse": {
                                    "toolUseId": tc.id,
                                    "name": tc.name,
                                    "input": tc.arguments,
                                }
                            }));
                        }
                        _ => {}
                    }
                }
                if !content_blocks.is_empty() {
                    bedrock_messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": content_blocks,
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
                let text = text_content.join("\n");

                let mut content: Vec<serde_json::Value> = vec![serde_json::json!({"text": text})];
                for block in &tr.content {
                    if let ContentBlock::Image(img) = block {
                        let format = match img.mime_type.as_str() {
                            "image/jpeg" | "image/jpg" => "jpeg",
                            "image/png" => "png",
                            "image/gif" => "gif",
                            "image/webp" => "webp",
                            _ => "png",
                        };
                        content.push(serde_json::json!({
                            "image": {
                                "format": format,
                                "source": {"bytes": img.data},
                            }
                        }));
                    }
                }

                bedrock_messages.push(serde_json::json!({
                    "role": "user",
                    "content": [{
                        "toolResult": {
                            "toolUseId": tr.tool_call_id,
                            "content": content,
                            "status": if tr.is_error { "error" } else { "success" },
                        }
                    }],
                }));
            }
        }
    }

    let mut body = serde_json::json!({
        "messages": bedrock_messages,
        "inferenceConfig": {},
    });

    if let Some(system) = &context.system_prompt {
        body["system"] = serde_json::json!([{"text": system}]);
    }
    if let Some(tools) = &context.tools {
        let tool_specs: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "toolSpec": {
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": {"json": t.parameters},
                    }
                })
            })
            .collect();
        body["toolConfig"] = serde_json::json!({
            "tools": tool_specs,
        });
    }

    let payload = serde_json::to_vec(&body).unwrap_or_default();
    (stream_url, payload)
}

async fn process_bedrock_stream(
    tx: tokio::sync::mpsc::Sender<StreamEvent>,
    stream_url: String,
    payload: Vec<u8>,
    access_key: String,
    secret_key: String,
    session_token: Option<String>,
    region: String,
    model_id: String,
    max_retries: u32,
    max_delay: u64,
) {
    let extra_headers = vec![("content-type".to_string(), "application/json".to_string())];

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let delay = crate::retry::retry_delay(attempt, 1000, max_delay);
            tokio::time::sleep(delay).await;
        }

        let auth_header = signing::build_aws_auth_header(
            "POST",
            &stream_url,
            &extra_headers,
            &payload,
            &access_key,
            &secret_key,
            session_token.as_deref(),
            &region,
        );

        let mut output = AssistantMessage::new(
            vec![],
            "bedrock-converse-stream".to_string(),
            "amazon-bedrock".to_string(),
            model_id.clone(),
            Usage::zero(),
            StopReason::Stop,
        );

        let _ = tx
            .send(StreamEvent::Start {
                partial: crate::types::PartialAssistantMessage {
                    content: vec![],
                    api: Some("bedrock-converse-stream".to_string()),
                    provider: Some("amazon-bedrock".to_string()),
                    model: Some(model_id.clone()),
                    usage: Some(Usage::zero()),
                    stop_reason: None,
                    error_message: None,
                    timestamp: chrono::Utc::now().timestamp_millis(),
                },
            })
            .await;

        let client = reqwest::Client::new();
        let x_amz_date = {
            let now = chrono::Utc::now();
            now.format("%Y%m%dT%H%M%SZ").to_string()
        };
        let mut request = client
            .post(&stream_url)
            .header("Authorization", &auth_header)
            .header("content-type", "application/json")
            .header("x-amz-date", &x_amz_date)
            .header("x-amz-content-sha256", signing::sha256_hex(&payload));

        if let Some(ref token) = session_token {
            request = request.header("x-amz-security-token", token);
        }

        request = request.body(payload.clone());

        match request.send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let status = resp.status();
                    let body_text = resp.text().await.unwrap_or_default();
                    let err_msg = format!("Bedrock API error ({}): {}", status, body_text);
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
                let mut text_blocks: std::collections::HashMap<usize, usize> =
                    std::collections::HashMap::new();
                let mut tool_blocks: std::collections::HashMap<usize, (String, String, String)> =
                    std::collections::HashMap::new();
                let mut saw_message_start = false;

                use futures::StreamExt;
                let mut chunk_stream = resp.bytes_stream();
                while let Some(chunk_result) = chunk_stream.next().await {
                    match chunk_result {
                        Ok(chunk) => {
                            let events = parser.feed(&chunk);
                            for sse in events {
                                events::process_bedrock_event(
                                    &tx,
                                    &sse,
                                    &mut output,
                                    &mut text_blocks,
                                    &mut tool_blocks,
                                    &mut saw_message_start,
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
        "bedrock-converse-stream".to_string(),
        "amazon-bedrock".to_string(),
        model_id,
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
}

/// Stream a response from AWS Bedrock Converse Stream API
pub fn stream_bedrock(
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
        let access_key = std::env::var("AWS_ACCESS_KEY_ID")
            .or_else(|_| std::env::var("AWS_ACCESS_KEY"))
            .unwrap_or_default();
        let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY").unwrap_or_default();
        let session_token = std::env::var("AWS_SESSION_TOKEN").ok();
        let region = std::env::var("AWS_REGION")
            .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
            .unwrap_or_else(|_| "us-east-1".to_string());

        if access_key.is_empty() || secret_key.is_empty() {
            let msg = AssistantMessage::new(
                vec![],
                "bedrock-converse-stream".to_string(),
                "amazon-bedrock".to_string(),
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

        let (stream_url, payload) = build_bedrock_request(&model, &context, &region);

        process_bedrock_stream(
            tx,
            stream_url,
            payload,
            access_key,
            secret_key,
            session_token,
            region,
            model.id.clone(),
            max_retries,
            max_delay,
        )
        .await;
    });

    rx
}

fn find_tool_call_index(
    output: &AssistantMessage,
    block_index: usize,
    tool_blocks: &std::collections::HashMap<usize, (String, String, String)>,
) -> Option<usize> {
    let tool_order: Vec<usize> = tool_blocks.keys().copied().collect();
    let pos = tool_order.iter().position(|k| *k == block_index)?;
    output
        .content
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, ContentBlock::ToolCall(_)))
        .nth(pos)
        .map(|(i, _)| i)
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
pub fn stream_simple_bedrock(
    model: Model,
    context: Context,
    _options: Option<crate::types::SimpleStreamOptions>,
) -> tokio::sync::mpsc::Receiver<StreamEvent> {
    stream_bedrock(model, context, None)
}
