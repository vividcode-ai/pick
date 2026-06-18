//! Model utility functions

use crate::types::{Api, KnownApi, KnownProvider, Model, ModelCost, Provider};

/// Get a list of known models
pub fn get_builtin_models() -> Vec<Model> {
    vec![
        Model {
            id: "claude-sonnet-4-6".to_string(),
            name: "Claude Sonnet 4.6".to_string(),
            api: Api::Known(KnownApi::AnthropicMessages),
            provider: Provider::Known(KnownProvider::Anthropic),
            base_url: "https://api.anthropic.com".to_string(),
            reasoning: true,
            thinking_level_map: None,
            input_capabilities: vec![
                crate::types::Capability::Text,
                crate::types::Capability::Image,
            ],
            cost: ModelCost {
                input: 3.0,
                output: 15.0,
                cache_read: 0.30,
                cache_write: 3.75,
            },
            context_window: 200_000,
            max_tokens: 8_192,
            headers: None,
            compat: None,
        },
        Model {
            id: "gpt-4o".to_string(),
            name: "GPT-4o".to_string(),
            api: Api::Known(KnownApi::OpenaiCompletions),
            provider: Provider::Known(KnownProvider::OpenAI),
            base_url: "https://api.openai.com".to_string(),
            reasoning: false,
            thinking_level_map: None,
            input_capabilities: vec![
                crate::types::Capability::Text,
                crate::types::Capability::Image,
            ],
            cost: ModelCost {
                input: 2.5,
                output: 10.0,
                cache_read: 1.25,
                cache_write: 2.5,
            },
            context_window: 128_000,
            max_tokens: 4_096,
            headers: None,
            compat: None,
        },
        Model {
            id: "faux-model".to_string(),
            name: "Faux Model".to_string(),
            api: Api::Custom("faux".to_string()),
            provider: Provider::Custom("faux".to_string()),
            base_url: "http://localhost".to_string(),
            reasoning: false,
            thinking_level_map: None,
            input_capabilities: vec![crate::types::Capability::Text],
            cost: ModelCost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: 4096,
            max_tokens: 1024,
            headers: None,
            compat: None,
        },
    ]
}

/// Compare two models for equality
pub fn models_are_equal(a: &Model, b: &Model) -> bool {
    a.id == b.id && a.provider == b.provider
}
