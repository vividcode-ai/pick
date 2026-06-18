//! Pick-ai: AI types, provider abstraction, and streaming.

pub mod image_models;
pub mod images;
pub mod images_openrouter;
pub mod models;
pub mod oauth;
pub mod providers;
pub mod registry;
pub mod retry;
pub mod sse;
pub mod types;
pub mod utils;

// Re-export core types at crate level - this makes `pick_ai::Message`, etc. work
pub use types::content::*;
pub use types::model::*;
pub use types::stream::*;
pub use types::tool::*;

// Re-export message types (Context, Message variants, etc.)
pub use types::message::*;

/// Result from a simple (non-streaming) completion
#[derive(Debug, Clone)]
pub struct SimpleCompletionResult {
    pub content: Vec<ContentBlock>,
    pub stop_reason: StopReason,
    pub error_message: Option<String>,
    pub usage: Usage,
}

/// Perform a simple text completion using a registered provider.
///
/// Looks up the provider by model.api, streams a response, and collects the result.
pub async fn complete_simple(
    model: &Model,
    context: Context,
    api_key: Option<String>,
    headers: Option<std::collections::HashMap<String, String>>,
    max_tokens: Option<u64>,
    temperature: Option<f64>,
    _reasoning: Option<String>,
) -> SimpleCompletionResult {
    let registry = registry::global_registry();
    let provider = registry.get(&model.api.as_str());

    let provider = match provider {
        Some(p) => p,
        None => {
            return SimpleCompletionResult {
                content: vec![],
                stop_reason: StopReason::Error,
                error_message: Some(format!(
                    "No provider registered for API: {}",
                    model.api.as_str()
                )),
                usage: Usage::zero(),
            };
        }
    };

    let mut options = StreamOptions::default();
    options.api_key = api_key;
    options.headers = headers;
    options.max_tokens = max_tokens;
    options.temperature = temperature;
    // reasoning is used by providers as a separate parameter, not in StreamOptions

    let mut receiver = (provider.stream)(model.clone(), context, Some(options));

    let mut content_parts: Vec<ContentBlock> = Vec::new();
    let mut stop_reason = StopReason::Stop;
    let mut error_message: Option<String> = None;
    let mut usage = Usage::zero();

    while let Some(event) = receiver.recv().await {
        match event {
            StreamEvent::Done { reason: _, message } => {
                content_parts = message.content;
                stop_reason = message.stop_reason;
                usage = message.usage;
                break;
            }
            StreamEvent::Error { reason, error } => {
                stop_reason = reason.clone();
                error_message = Some(
                    error
                        .error_message
                        .unwrap_or_else(|| format!("{:?}", reason)),
                );
                content_parts = error.content;
                usage = error.usage;
                break;
            }
            _ => {}
        }
    }

    SimpleCompletionResult {
        content: content_parts,
        stop_reason,
        error_message,
        usage,
    }
}
