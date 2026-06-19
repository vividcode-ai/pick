//! Streaming logic and event processing

use pick_ai::registry::global_registry;
use pick_ai::types::{
    AssistantMessage, Context, Message, Model, StopReason, StreamEvent, StreamOptions, Usage,
};

use super::super::events::{AgentEvent, AgentEventHandler};
use super::super::state::ThinkingLevel;

/// Map a ThinkingLevel to a token budget
fn thinking_budget_from_level(level: ThinkingLevel) -> Option<u64> {
    match level {
        ThinkingLevel::Off => None,
        ThinkingLevel::Minimal => Some(1024),
        ThinkingLevel::Low => Some(2048),
        ThinkingLevel::Medium => Some(8192),
        ThinkingLevel::High => Some(16384),
        ThinkingLevel::XHigh => Some(32768),
    }
}

/// Make a single LLM call
pub async fn call_llm(
    model: &Model,
    context: Context,
    on_event: Option<&AgentEventHandler>,
    cancel_signal: Option<std::sync::Arc<tokio::sync::watch::Receiver<bool>>>,
    thinking_level: ThinkingLevel,
    api_key_override: Option<String>,
    provider_max_retries: Option<u32>,
    provider_max_retry_delay_ms: Option<u64>,
) -> Result<AssistantMessage, String> {
    let registry = global_registry();
    let api_key = model.api.as_str();

    // Find the registered provider
    let provider = registry
        .get(api_key)
        .ok_or_else(|| format!("No provider registered for API: {}", api_key))?;

    let thinking_budget = thinking_budget_from_level(thinking_level);
    // When thinking is enabled, max_tokens must include room for both thinking and visible output
    let max_tokens = thinking_budget.map(|budget| model.max_tokens + budget);

    let stream_options = StreamOptions {
        temperature: None,
        max_tokens,
        api_key: api_key_override,
        transport: None,
        cache_retention: None,
        session_id: None,
        headers: None,
        timeout_ms: None,
        max_retries: provider_max_retries.or(Some(3)),
        max_retry_delay_ms: provider_max_retry_delay_ms,
        thinking_budget,
        metadata: None,
        signal: cancel_signal.clone(),
    };

    let mut receiver = (provider.stream)(model.clone(), context, Some(stream_options));

    let mut result_msg = None;

    while let Some(event) = receiver.recv().await {
        // Check for cancellation request during streaming
        if let Some(ref sig) = cancel_signal
            && *sig.borrow()
        {
            return Err("LLM call cancelled".to_string());
        }

        match event {
            StreamEvent::Done { message, .. } => {
                result_msg = Some(message);
                break;
            }
            StreamEvent::Error { reason, error } => {
                return Err(error
                    .error_message
                    .unwrap_or_else(|| format!("{:?}", reason)));
            }
            other => {
                // Forward intermediate stream events as MessageUpdate for UI streaming
                if let Some(handler) = on_event
                    && let Some(msg) = partial_event_to_message(&other)
                {
                    handler(AgentEvent::MessageUpdate {
                        message: msg,
                        assistant_message_event: None,
                    });
                }
            }
        }
    }

    result_msg.ok_or_else(|| "No response from LLM".to_string())
}

/// Convert a StreamEvent with partial content into a Message for UI streaming updates
fn partial_event_to_message(event: &StreamEvent) -> Option<Message> {
    let partial = match event {
        StreamEvent::Start { partial }
        | StreamEvent::TextStart { partial, .. }
        | StreamEvent::TextDelta { partial, .. }
        | StreamEvent::TextEnd { partial, .. }
        | StreamEvent::ThinkingStart { partial, .. }
        | StreamEvent::ThinkingDelta { partial, .. }
        | StreamEvent::ThinkingEnd { partial, .. }
        | StreamEvent::ToolCallStart { partial, .. }
        | StreamEvent::ToolCallDelta { partial, .. }
        | StreamEvent::ToolCallEnd { partial, .. } => partial,
        StreamEvent::Done { .. } | StreamEvent::Error { .. } => return None,
    };

    Some(Message::Assistant(AssistantMessage::new(
        partial.content.clone(),
        partial.api.clone().unwrap_or_default(),
        partial.provider.clone().unwrap_or_default(),
        partial.model.clone().unwrap_or_default(),
        partial.usage.clone().unwrap_or_else(Usage::zero),
        partial
            .stop_reason
            .as_deref()
            .map(|s| match s {
                "stop" => StopReason::Stop,
                "length" => StopReason::Length,
                "toolUse" => StopReason::ToolUse,
                "error" => StopReason::Error,
                "aborted" => StopReason::Aborted,
                _ => StopReason::Stop,
            })
            .unwrap_or(StopReason::Stop),
    )))
}
