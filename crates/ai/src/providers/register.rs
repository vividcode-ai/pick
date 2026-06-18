//! Register built-in API providers

use crate::registry::{ApiProviderRegistry, RegisteredProvider};
use super::{
    anthropic, openai, faux,
    mistral, google, google_vertex,
    openai_responses, azure_openai_responses, openai_codex_responses,
    bedrock,
};

/// Register all built-in API providers (internal, called from registry init)
pub fn register_builtins_internal(registry: &ApiProviderRegistry) {
    registry.register(RegisteredProvider {
        api: "anthropic-messages".to_string(),
        stream: std::sync::Arc::new(|model, context, options| {
            anthropic::stream_anthropic(model, context, options)
        }),
        source_id: Some("builtin".to_string()),
    });

    registry.register(RegisteredProvider {
        api: "openai-completions".to_string(),
        stream: std::sync::Arc::new(|model, context, options| {
            openai::stream_openai_completions(model, context, options)
        }),
        source_id: Some("builtin".to_string()),
    });

    registry.register(RegisteredProvider {
        api: "faux".to_string(),
        stream: std::sync::Arc::new(|model, context, options| {
            faux::stream_faux(model, context, options)
        }),
        source_id: Some("builtin".to_string()),
    });

    registry.register(RegisteredProvider {
        api: "mistral-conversations".to_string(),
        stream: std::sync::Arc::new(|model, context, options| {
            mistral::stream_mistral(model, context, options)
        }),
        source_id: Some("builtin".to_string()),
    });

    registry.register(RegisteredProvider {
        api: "google-generative-ai".to_string(),
        stream: std::sync::Arc::new(|model, context, options| {
            google::stream_google(model, context, options)
        }),
        source_id: Some("builtin".to_string()),
    });

    registry.register(RegisteredProvider {
        api: "google-vertex".to_string(),
        stream: std::sync::Arc::new(|model, context, options| {
            google_vertex::stream_google_vertex(model, context, options)
        }),
        source_id: Some("builtin".to_string()),
    });

    registry.register(RegisteredProvider {
        api: "openai-responses".to_string(),
        stream: std::sync::Arc::new(|model, context, options| {
            openai_responses::stream_openai_responses(model, context, options)
        }),
        source_id: Some("builtin".to_string()),
    });

    registry.register(RegisteredProvider {
        api: "azure-openai-responses".to_string(),
        stream: std::sync::Arc::new(|model, context, options| {
            azure_openai_responses::stream_azure_openai_responses(model, context, options)
        }),
        source_id: Some("builtin".to_string()),
    });

    registry.register(RegisteredProvider {
        api: "openai-codex-responses".to_string(),
        stream: std::sync::Arc::new(|model, context, options| {
            openai_codex_responses::stream_openai_codex_responses(model, context, options)
        }),
        source_id: Some("builtin".to_string()),
    });

    registry.register(RegisteredProvider {
        api: "bedrock-converse-stream".to_string(),
        stream: std::sync::Arc::new(|model, context, options| {
            bedrock::stream_bedrock(model, context, options)
        }),
        source_id: Some("builtin".to_string()),
    });
}

/// Register all built-in providers (public API)
pub fn register_builtins() {
    crate::registry::global_registry();
}
