//! Model registry - model definitions and lookup functions

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::types::{Model, ThinkingLevel, Usage};

// Include the auto-generated model registry (produced by build.rs)
include!(concat!(env!("OUT_DIR"), "/models_generated.rs"));

/// Get a model by provider and model ID
pub fn get_model(provider: &str, model_id: &str) -> Option<Model> {
    MODEL_REGISTRY
        .get(provider)
        .and_then(|models| models.get(model_id))
        .cloned()
}

/// Get all known provider names
pub fn get_providers() -> Vec<String> {
    MODEL_REGISTRY.keys().cloned().collect()
}

/// Get all models for a provider
pub fn get_models(provider: &str) -> Vec<Model> {
    MODEL_REGISTRY
        .get(provider)
        .map(|models| models.values().cloned().collect())
        .unwrap_or_default()
}

/// Calculate cost from model pricing and usage (mutates usage in place)
/// Cost is in dollars, model cost fields are per-million-tokens
pub fn calculate_cost(model: &Model, usage: &mut Usage) {
    usage.cost.input = (model.cost.input / 1_000_000.0) * usage.input as f64;
    usage.cost.output = (model.cost.output / 1_000_000.0) * usage.output as f64;
    usage.cost.cache_read = (model.cost.cache_read / 1_000_000.0) * usage.cache_read as f64;
    usage.cost.cache_write = (model.cost.cache_write / 1_000_000.0) * usage.cache_write as f64;
    usage.cost.total =
        usage.cost.input + usage.cost.output + usage.cost.cache_read + usage.cost.cache_write;
}

/// Get supported thinking levels for a model
pub fn get_supported_thinking_levels(model: &Model) -> Vec<ThinkingLevel> {
    if !model.reasoning {
        return vec![ThinkingLevel::Off];
    }

    let all_levels = [
        ThinkingLevel::Off,
        ThinkingLevel::Minimal,
        ThinkingLevel::Low,
        ThinkingLevel::Medium,
        ThinkingLevel::High,
        ThinkingLevel::XHigh,
    ];

    if let Some(ref tlm) = model.thinking_level_map {
        all_levels
            .into_iter()
            .filter(|level| {
                if *level == ThinkingLevel::XHigh {
                    tlm.get("xhigh").and_then(|v| v.as_ref()).is_some()
                } else {
                    let key = match level {
                        ThinkingLevel::Off => "off",
                        ThinkingLevel::Minimal => "minimal",
                        ThinkingLevel::Low => "low",
                        ThinkingLevel::Medium => "medium",
                        ThinkingLevel::High => "high",
                        ThinkingLevel::XHigh => "xhigh",
                    };
                    tlm.get(key).map(|v| v.is_some()).unwrap_or(true)
                }
            })
            .collect()
    } else {
        all_levels.to_vec()
    }
}

/// Clamp a thinking level to the nearest available level for a model
pub fn clamp_thinking_level(model: &Model, level: ThinkingLevel) -> ThinkingLevel {
    let available = get_supported_thinking_levels(model);
    if available.contains(&level) {
        return level;
    }

    let all_levels = [
        ThinkingLevel::Off,
        ThinkingLevel::Minimal,
        ThinkingLevel::Low,
        ThinkingLevel::Medium,
        ThinkingLevel::High,
        ThinkingLevel::XHigh,
    ];

    let requested_idx = all_levels.iter().position(|l| *l == level).unwrap_or(0);

    // Search up from requested level
    for i in requested_idx..all_levels.len() {
        if available.contains(&all_levels[i]) {
            return all_levels[i];
        }
    }

    // Search down from requested level
    for i in (0..requested_idx).rev() {
        if available.contains(&all_levels[i]) {
            return all_levels[i];
        }
    }

    available.first().copied().unwrap_or(ThinkingLevel::Off)
}

/// Check if two models are equal by id and provider
pub fn models_are_equal(a: Option<&Model>, b: Option<&Model>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.id == b.id && a.provider == b.provider,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_anthropic_model() {
        let model = get_model("anthropic", "claude-sonnet-4-20250514").unwrap();
        assert_eq!(model.provider.as_str(), "anthropic");
        assert_eq!(model.api.as_str(), "anthropic-messages");
    }

    #[test]
    fn test_get_openai_model() {
        let model = get_model("openai", "gpt-4o").unwrap();
        assert_eq!(model.provider.as_str(), "openai");
        assert_eq!(model.api.as_str(), "openai-responses");
    }

    #[test]
    fn test_get_mistral_model() {
        let model = get_model("mistral", "mistral-large-2411").unwrap();
        assert_eq!(model.provider.as_str(), "mistral");
        assert_eq!(model.api.as_str(), "mistral-conversations");
    }

    #[test]
    fn test_get_google_model() {
        let model = get_model("google", "gemini-2.5-pro").unwrap();
        assert_eq!(model.provider.as_str(), "google");
        assert_eq!(model.api.as_str(), "google-generative-ai");
    }

    #[test]
    fn test_get_bedrock_model() {
        let model = get_model(
            "amazon-bedrock",
            "anthropic.claude-sonnet-4-5-20250929-v1:0",
        )
        .unwrap();
        assert_eq!(model.provider.as_str(), "amazon-bedrock");
        assert_eq!(model.api.as_str(), "bedrock-converse-stream");
    }

    #[test]
    fn test_unknown_model() {
        assert!(get_model("anthropic", "nonexistent-model").is_none());
    }

    #[test]
    fn test_get_providers() {
        let providers = get_providers();
        assert!(
            providers.contains(&"anthropic".to_string()),
            "anthropic not found: {:?}",
            providers
        );
        assert!(
            providers.contains(&"openai".to_string()),
            "openai not found: {:?}",
            providers
        );
        assert!(
            providers.contains(&"mistral".to_string()),
            "mistral not found: {:?}",
            providers
        );
        assert!(
            providers.contains(&"google".to_string()),
            "google not found: {:?}",
            providers
        );
        assert!(
            providers.contains(&"google-vertex".to_string()),
            "google-vertex not found: {:?}",
            providers
        );
        assert!(
            providers.contains(&"amazon-bedrock".to_string()),
            "amazon-bedrock not found: {:?}",
            providers
        );
    }

    #[test]
    fn test_calculate_cost() {
        let model = get_model("openai", "gpt-4o").unwrap();
        let mut usage = Usage {
            input: 1000,
            output: 500,
            cache_read: 0,
            cache_write: 0,
            total_tokens: 1500,
            cost: crate::types::CostBreakdown::zero(),
        };
        calculate_cost(&model, &mut usage);
        assert!(usage.cost.input > 0.0);
        assert!(usage.cost.output > 0.0);
        assert!(usage.cost.total > 0.0);
    }

    #[test]
    fn test_thinking_levels_non_reasoning() {
        let model = get_model("openai", "gpt-4o").unwrap();
        let levels = get_supported_thinking_levels(&model);
        assert_eq!(levels, vec![ThinkingLevel::Off]);
    }

    #[test]
    fn test_thinking_levels_reasoning() {
        let model = get_model("anthropic", "claude-sonnet-4-20250514").unwrap();
        let levels = get_supported_thinking_levels(&model);
        assert!(levels.contains(&ThinkingLevel::Off));
        assert!(levels.contains(&ThinkingLevel::High));
    }

    #[test]
    fn test_models_equal() {
        let a = get_model("anthropic", "claude-sonnet-4-20250514");
        let b = get_model("anthropic", "claude-sonnet-4-20250514");
        assert!(models_are_equal(a.as_ref(), b.as_ref()));

        let c = get_model("openai", "gpt-4o");
        assert!(!models_are_equal(a.as_ref(), c.as_ref()));
    }
}
