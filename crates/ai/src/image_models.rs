//! Image model registry.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::images::ImagesModel;
use crate::types::ModelCost;

static IMAGE_MODEL_REGISTRY: LazyLock<HashMap<String, HashMap<String, ImagesModel>>> =
    LazyLock::new(|| {
        let mut registry = HashMap::new();

        // OpenRouter models
        let mut openrouter = HashMap::new();
        for (
            id,
            name,
            input_caps,
            output_caps,
            cost_input,
            cost_output,
            cost_cache_read,
            cost_cache_write,
        ) in [
            (
                "black-forest-labs/flux.2-flex",
                "Black Forest Labs: FLUX.2 Flex",
                &["text", "image"] as &[&str],
                &["image"] as &[&str],
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            (
                "black-forest-labs/flux.2-klein-4b",
                "Black Forest Labs: FLUX.2 Klein 4B",
                &["text", "image"],
                &["image"],
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            (
                "black-forest-labs/flux.2-max",
                "Black Forest Labs: FLUX.2 Max",
                &["text", "image"],
                &["image"],
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            (
                "black-forest-labs/flux.2-pro",
                "Black Forest Labs: FLUX.2 Pro",
                &["text", "image"],
                &["image"],
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            (
                "bytedance-seed/seedream-4.5",
                "ByteDance Seed: Seedream 4.5",
                &["image", "text"],
                &["image"],
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            (
                "google/gemini-2.5-flash-image",
                "Google: Nano Banana (Gemini 2.5 Flash Image)",
                &["image", "text"],
                &["image", "text"],
                0.3,
                2.5,
                0.03,
                0.08333333333333334,
            ),
            (
                "google/gemini-3-pro-image-preview",
                "Google: Nano Banana Pro (Gemini 3 Pro Image Preview)",
                &["image", "text"],
                &["image", "text"],
                2.0,
                12.0,
                0.2,
                0.375,
            ),
            (
                "google/gemini-3.1-flash-image-preview",
                "Google: Nano Banana 2 (Gemini 3.1 Flash Image Preview)",
                &["image", "text"],
                &["image", "text"],
                0.5,
                3.0,
                0.0,
                0.0,
            ),
            (
                "openai/gpt-5-image",
                "OpenAI: GPT-5 Image",
                &["image", "text"],
                &["image", "text"],
                10.0,
                10.0,
                1.25,
                0.0,
            ),
            (
                "openai/gpt-5-image-mini",
                "OpenAI: GPT-5 Image Mini",
                &["image", "text"],
                &["image", "text"],
                2.5,
                2.0,
                0.25,
                0.0,
            ),
            (
                "openai/gpt-5.4-image-2",
                "OpenAI: GPT-5.4 Image 2",
                &["image", "text"],
                &["image", "text"],
                8.0,
                15.0,
                2.0,
                0.0,
            ),
            (
                "openrouter/auto",
                "Auto Router",
                &["text", "image"],
                &["text", "image"],
                -1000000.0,
                -1000000.0,
                0.0,
                0.0,
            ),
            (
                "recraft/recraft-v3",
                "Recraft: Recraft V3",
                &["text", "image"],
                &["image"],
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            (
                "recraft/recraft-v4",
                "Recraft: Recraft V4",
                &["text", "image"],
                &["image"],
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            (
                "recraft/recraft-v4-pro",
                "Recraft: Recraft V4 Pro",
                &["text", "image"],
                &["image"],
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            (
                "recraft/recraft-v4-pro-vector",
                "Recraft: Recraft V4 Pro Vector",
                &["text", "image"],
                &["image"],
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            (
                "recraft/recraft-v4-vector",
                "Recraft: Recraft V4 Vector",
                &["text", "image"],
                &["image"],
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            (
                "recraft/recraft-v4.1",
                "Recraft: Recraft V4.1",
                &["text", "image"],
                &["image"],
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            (
                "recraft/recraft-v4.1-pro",
                "Recraft: Recraft V4.1 Pro",
                &["text", "image"],
                &["image"],
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            (
                "recraft/recraft-v4.1-pro-vector",
                "Recraft: Recraft V4.1 Pro Vector",
                &["text", "image"],
                &["image"],
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            (
                "recraft/recraft-v4.1-utility",
                "Recraft: Recraft V4.1 Utility",
                &["text", "image"],
                &["image"],
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            (
                "recraft/recraft-v4.1-utility-pro",
                "Recraft: Recraft V4.1 Utility Pro",
                &["text", "image"],
                &["image"],
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            (
                "recraft/recraft-v4.1-vector",
                "Recraft: Recraft V4.1 Vector",
                &["text", "image"],
                &["image"],
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            (
                "x-ai/grok-imagine-image-quality",
                "xAI: Grok Imagine Image Quality",
                &["text", "image"],
                &["image"],
                0.0,
                0.0,
                0.0,
                0.0,
            ),
        ] {
            openrouter.insert(
                id.to_string(),
                ImagesModel {
                    id: id.to_string(),
                    name: name.to_string(),
                    api: crate::images::KNOWN_IMAGES_API_OPENROUTER.to_string(),
                    provider: "openrouter".to_string(),
                    base_url: "https://openrouter.ai/api/v1".to_string(),
                    input_capabilities: Some(input_caps.iter().map(|s| s.to_string()).collect()),
                    output_capabilities: output_caps.iter().map(|s| s.to_string()).collect(),
                    cost: ModelCost {
                        input: cost_input,
                        output: cost_output,
                        cache_read: cost_cache_read,
                        cache_write: cost_cache_write,
                    },
                    headers: None,
                },
            );
        }
        registry.insert("openrouter".to_string(), openrouter);

        registry
    });

/// Get an image model by provider and model ID.
pub fn get_image_model(provider: &str, model_id: &str) -> Option<ImagesModel> {
    IMAGE_MODEL_REGISTRY.get(provider)?.get(model_id).cloned()
}

/// Get all registered image providers.
pub fn get_image_providers() -> Vec<String> {
    IMAGE_MODEL_REGISTRY.keys().cloned().collect()
}

/// Get all models for a given provider.
pub fn get_image_models(provider: &str) -> Vec<ImagesModel> {
    IMAGE_MODEL_REGISTRY
        .get(provider)
        .map(|models| models.values().cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_openrouter_model() {
        let model = get_image_model("openrouter", "google/gemini-2.5-flash-image");
        assert!(model.is_some());
        let m = model.unwrap();
        assert_eq!(m.id, "google/gemini-2.5-flash-image");
        assert_eq!(m.provider, "openrouter");
        assert_eq!(m.api, crate::images::KNOWN_IMAGES_API_OPENROUTER);
    }

    #[test]
    fn test_get_unknown_model() {
        assert!(get_image_model("openrouter", "nonexistent-model").is_none());
        assert!(get_image_model("unknown-provider", "anything").is_none());
    }

    #[test]
    fn test_get_providers() {
        let providers = get_image_providers();
        assert!(providers.contains(&"openrouter".to_string()));
    }

    #[test]
    fn test_get_models() {
        let models = get_image_models("openrouter");
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id.contains("gemini")));
    }
}
