//! Anthropic Messages API provider with SSE streaming

pub(crate) mod events;

use crate::providers::cloudflare;
use crate::types::{
    AssistantMessage, Context, Model, StopReason, StreamEvent, StreamOptions, Usage,
};

use events::BlockState;

/// Stream a response from Anthropic's Messages API with full SSE parsing
pub fn stream_anthropic(
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
            .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
            .or_else(|| std::env::var("ANTHROPIC_KEY").ok())
            .unwrap_or_default();

        if api_key.is_empty() {
            let mut msg = AssistantMessage::new(
                vec![],
                "anthropic-messages".to_string(),
                "anthropic".to_string(),
                model.id.clone(),
                Usage::zero(),
                StopReason::Error,
            );
            msg.error_message = Some(
                "ANTHROPIC_API_KEY is not set. Set the ANTHROPIC_API_KEY environment variable."
                    .to_string(),
            );
            let _ = tx
                .send(StreamEvent::Error {
                    reason: StopReason::Error,
                    error: msg,
                })
                .await;
            return;
        }

        let base_url = if cloudflare::is_cloudflare_ai_gateway(&model.provider) {
            match cloudflare::resolve_cloudflare_base_url(&model.base_url) {
                Ok(url) => url,
                Err(_) => {
                    let msg = AssistantMessage::new(
                        vec![],
                        "anthropic-messages".to_string(),
                        "anthropic".to_string(),
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
        let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));

        let messages: Vec<serde_json::Value> = context
            .messages
            .iter()
            .map(|m| serde_json::to_value(m).unwrap_or_default())
            .collect();

        let opts = options.as_ref();

        let max_tokens = opts.and_then(|o| o.max_tokens).unwrap_or(4096);

        let mut body = serde_json::json!({
            "model": model.id,
            "max_tokens": max_tokens,
            "stream": true,
            "messages": messages,
        });

        // Build thinking config from options.
        // Priority:
        //   1. If reasoning string is provided, use it to map through thinking_level_map
        //      to determine the appropriate thinking mode (adaptive, budget, or disabled).
        //   2. Fall back to the raw thinking_budget (legacy behavior).
        //   3. If neither is set and the model supports reasoning, default to disabled.
        if let Some(reasoning_str) = opts.and_then(|o| o.reasoning.as_ref()) {
            let is_adaptive = model
                .compat
                .as_ref()
                .and_then(|c| c.anthropic_messages.as_ref())
                .and_then(|a| a.force_adaptive_thinking)
                .unwrap_or(false);

            if reasoning_str == "off"
                && model
                    .thinking_level_map
                    .as_ref()
                    .and_then(|tlm| tlm.get("off"))
                    .map(|v| v.is_some())
                    .unwrap_or(true)
            {
                // Explicitly disabled — only send if model allows it
                body["thinking"] = serde_json::json!({ "type": "disabled" });
            } else if reasoning_str == "off" {
                // Model does not support disabling thinking (e.g. claude-opus-4-7) — skip param
            } else if is_adaptive {
                // Adaptive thinking: e.g. claude-opus-4-7 with output_config.effort
                let effort = model
                    .thinking_level_map
                    .as_ref()
                    .and_then(|tlm| tlm.get(reasoning_str))
                    .and_then(|v| v.as_ref())
                    .map(|s| s.as_str())
                    .unwrap_or(reasoning_str);
                body["thinking"] = serde_json::json!({
                    "type": "adaptive",
                    "display": "summarized"
                });
                body["output_config"] = serde_json::json!({
                    "effort": effort
                });
            } else if model.reasoning {
                // Legacy token-budget-based thinking
                let budget = opts.and_then(|o| o.thinking_budget).unwrap_or(8192);
                body["thinking"] = serde_json::json!({
                    "type": "enabled",
                    "budget_tokens": budget
                });
            }
        } else if let Some(budget) = opts.and_then(|o| o.thinking_budget) {
            // Legacy: raw token budget without reasoning string
            body["thinking"] = serde_json::json!({
                "type": "enabled",
                "budget_tokens": budget
            });
        }

        if let Some(system) = &context.system_prompt {
            body["system"] = serde_json::json!(system);
        }
        if let Some(tools) = &context.tools {
            body["tools"] = serde_json::to_value(tools).unwrap_or_default();
        }

        let is_cloudflare_gateway = cloudflare::is_cloudflare_ai_gateway(&model.provider);

        // Determine if interleaved-thinking beta header is needed (legacy budget-based thinking)
        let use_interleaved_thinking = model.reasoning
            && !model
                .compat
                .as_ref()
                .and_then(|c| c.anthropic_messages.as_ref())
                .and_then(|a| a.force_adaptive_thinking)
                .unwrap_or(false);

        for attempt in 0..=max_retries {
            if let Some(signal) = opts.and_then(|o| o.signal.as_ref())
                && *signal.borrow()
            {
                let mut msg = AssistantMessage::new(
                    vec![],
                    "anthropic-messages".to_string(),
                    "anthropic".to_string(),
                    model.id.clone(),
                    Usage::zero(),
                    StopReason::Aborted,
                );
                msg.error_message = Some("LLM call cancelled before request".to_string());
                let _ = tx
                    .send(StreamEvent::Error {
                        reason: StopReason::Aborted,
                        error: msg,
                    })
                    .await;
                return;
            }

            if attempt > 0 {
                let delay = crate::retry::retry_delay(attempt, 1000, max_delay);
                tokio::time::sleep(delay).await;
            }

            let mut output = AssistantMessage::new(
                vec![],
                "anthropic-messages".to_string(),
                "anthropic".to_string(),
                model.id.clone(),
                Usage::zero(),
                StopReason::Stop,
            );

            let _ = tx
                .send(StreamEvent::Start {
                    partial: crate::types::PartialAssistantMessage {
                        content: vec![],
                        api: Some("anthropic-messages".to_string()),
                        provider: Some("anthropic".to_string()),
                        model: Some(model.id.clone()),
                        usage: Some(Usage::zero()),
                        stop_reason: None,
                        error_message: None,
                        timestamp: chrono::Utc::now().timestamp_millis(),
                    },
                })
                .await;

            let client = reqwest::Client::new();
            let mut request_builder = client
                .post(&url)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json");
            // Add interleaved-thinking beta header for legacy budget-based thinking
            if use_interleaved_thinking {
                request_builder =
                    request_builder.header("anthropic-beta", "interleaved-thinking-2");
            }
            let request_builder = if is_cloudflare_gateway {
                request_builder.header("cf-aig-authorization", format!("Bearer {}", api_key))
            } else {
                request_builder.header("x-api-key", &api_key)
            };
            match request_builder.json(&body).send().await {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        let status = resp.status();
                        let body_text = resp.text().await.unwrap_or_default();
                        let err_msg = format!("Anthropic API error ({}): {}", status, body_text);
                        if crate::retry::is_retryable_http_status(status.as_u16())
                            && crate::retry::should_retry(attempt, max_retries)
                        {
                            continue;
                        }
                        emit_error(&tx, &mut output, &model, &err_msg).await;
                        return;
                    }

                    let mut parser = crate::sse::SseParser::new();
                    let mut blocks: Vec<BlockState> = Vec::new();
                    let mut saw_message_start = false;
                    let mut saw_message_stop = false;
                    let mut stream_error: Option<String> = None;

                    let mut chunk_stream = resp.bytes_stream();

                    use futures::StreamExt;
                    while let Some(chunk_result) = chunk_stream.next().await {
                        if let Some(signal) = opts.and_then(|o| o.signal.as_ref())
                            && *signal.borrow()
                        {
                            emit_error(
                                &tx,
                                &mut output,
                                &model,
                                "LLM call cancelled during streaming",
                            )
                            .await;
                            return;
                        }

                        match chunk_result {
                            Ok(chunk) => {
                                let events = parser.feed(&chunk);
                                for sse_event in events {
                                    if let Err(e) = events::process_anthropic_event(
                                        &tx,
                                        sse_event,
                                        &mut output,
                                        &mut blocks,
                                        &mut saw_message_start,
                                        &mut saw_message_stop,
                                        &mut stream_error,
                                    )
                                    .await
                                    {
                                        stream_error = Some(e);
                                    }
                                }
                            }
                            Err(e) => {
                                stream_error = Some(format!("Stream read error: {}", e));
                                break;
                            }
                        }
                    }

                    let remaining = parser.finish();
                    for sse_event in remaining {
                        if let Err(e) = events::process_anthropic_event(
                            &tx,
                            sse_event,
                            &mut output,
                            &mut blocks,
                            &mut saw_message_start,
                            &mut saw_message_stop,
                            &mut stream_error,
                        )
                        .await
                        {
                            stream_error = Some(e);
                        }
                    }

                    if let Some(err) = stream_error {
                        emit_error(&tx, &mut output, &model, &err).await;
                        return;
                    }

                    if saw_message_start && !saw_message_stop {
                        emit_error(
                            &tx,
                            &mut output,
                            &model,
                            "Anthropic stream ended before message_stop",
                        )
                        .await;
                        return;
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
                    emit_error(&tx, &mut output, &model, &format!("Request failed: {}", e)).await;
                    return;
                }
            }
        }

        let mut msg = AssistantMessage::new(
            vec![],
            "anthropic-messages".to_string(),
            "anthropic".to_string(),
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

async fn emit_error(
    tx: &tokio::sync::mpsc::Sender<StreamEvent>,
    output: &mut AssistantMessage,
    _model: &Model,
    msg: &str,
) {
    output.stop_reason = StopReason::Error;
    output.error_message = Some(msg.to_string());
    let _ = tx
        .send(StreamEvent::Error {
            reason: StopReason::Error,
            error: output.clone(),
        })
        .await;
}

/// Map Anthropic stop reasons to our StopReason enum
pub(crate) fn map_anthropic_stop_reason(reason: &str) -> StopReason {
    match reason {
        "end_turn" => StopReason::Stop,
        "max_tokens" => StopReason::Length,
        "stop_sequence" => StopReason::Stop,
        "tool_use" => StopReason::ToolUse,
        "refusal" => StopReason::Stop,
        "pause_turn" => StopReason::Stop,
        "sensitive" => StopReason::Error,
        _ => StopReason::Stop,
    }
}

/// Build a PartialAssistantMessage from current output state
pub(crate) fn partial_from_output(
    output: &AssistantMessage,
) -> crate::types::PartialAssistantMessage {
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

/// Try to parse JSON, returning Null on failure
pub(crate) fn try_parse_json(s: &str) -> serde_json::Value {
    serde_json::from_str(s).unwrap_or(serde_json::Value::Null)
}

/// Simple streaming version (with reasoning support)
pub fn stream_simple_anthropic(
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
    stream_anthropic(model, context, stream_opts)
}
