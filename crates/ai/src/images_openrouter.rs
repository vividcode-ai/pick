//! OpenRouter image generation provider.

use crate::images::{
    AssistantImages, ImagesContext, ImagesImageContent, ImagesModel, ImagesOptions,
    ImagesOutputContent, ImagesStopReason, ImagesTextContent, KNOWN_IMAGES_API_OPENROUTER,
    ProviderResponse,
};
use crate::utils::sanitize_unicode::sanitize_unicode;

/// Generate images using OpenRouter's chat completions API.
/// Uses the OpenAI-compatible endpoint with image generation modalities.
pub async fn generate_images_openrouter(
    model: ImagesModel,
    context: ImagesContext,
    options: Option<ImagesOptions>,
) -> Result<AssistantImages, String> {
    let mut output = AssistantImages {
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        output: Vec::new(),
        response_id: None,
        usage: None,
        stop_reason: ImagesStopReason::Stop,
        error_message: None,
        timestamp: chrono::Utc::now().timestamp_millis(),
    };

    let api_key = match options.as_ref().and_then(|o| o.api_key.as_ref()) {
        Some(key) => key.clone(),
        None => std::env::var("OPENROUTER_API_KEY")
            .or_else(|_| std::env::var("OPENAI_API_KEY"))
            .map_err(|_| format!("No API key available for provider: {}", model.provider))?,
    };

    let base_url = model.base_url.trim_end_matches('/').to_string();
    let url = format!("{}/v1/chat/completions", base_url);

    // Build request payload
    let mut messages_content: Vec<serde_json::Value> = Vec::new();
    for input_item in &context.input {
        match input_item {
            crate::images::ImagesInputContent::Text(t) => {
                messages_content.push(serde_json::json!({
                    "type": "text",
                    "text": sanitize_unicode(&t.text),
                }));
            }
            crate::images::ImagesInputContent::Image(img) => {
                messages_content.push(serde_json::json!({
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:{};base64,{}", img.mime_type, img.data),
                    },
                }));
            }
        }
    }

    let supports_text = model.output_capabilities.iter().any(|c| c == "text");
    let modalities: Vec<&str> = if supports_text {
        vec!["image", "text"]
    } else {
        vec!["image"]
    };

    let mut request_body = serde_json::json!({
        "model": model.id,
        "messages": [
            {
                "role": "user",
                "content": messages_content,
            }
        ],
        "stream": false,
        "modalities": modalities,
    });

    // Extract needed values from options before the async call
    let on_payload = options.as_ref().and_then(|o| o.on_payload.clone());
    let custom_headers = options.as_ref().and_then(|o| o.headers.clone());
    let timeout_ms = options.as_ref().and_then(|o| o.timeout_ms);

    // Allow on_payload callback to inspect/modify the payload
    if let Some(ref on_payload_fn) = on_payload
        && let Some(modified) = on_payload_fn(&request_body, &model) {
            request_body = modified;
        }

    // Build HTTP client and request
    let client = reqwest::Client::new();
    let mut req_builder = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body);

    if let Some(ref headers) = custom_headers {
        for (key, value) in headers.iter() {
            req_builder = req_builder.header(key.as_str(), value.as_str());
        }
    }

    if let Some(timeout) = timeout_ms {
        req_builder = req_builder.timeout(std::time::Duration::from_millis(timeout));
    }

    let on_response = options.as_ref().and_then(|o| o.on_response.clone());

    let response = req_builder
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    let status = response.status().as_u16();
    let response_headers = response.headers().clone();
    let response_text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    // Build ProviderResponse for callback
    let provider_response = ProviderResponse {
        status,
        headers: response_headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect(),
    };

    if let Some(ref on_response_fn) = on_response {
        on_response_fn(&provider_response, &model);
    }

    if status != 200 {
        output.stop_reason = ImagesStopReason::Error;
        output.error_message = Some(format!("HTTP {}: {}", status, response_text));
        return Ok(output);
    }

    // Parse response
    let json: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
        output.response_id = Some(id.to_string());
    }

    // Parse usage
    if let Some(usage) = json.get("usage") {
        output.usage = Some(parse_usage(usage, &model));
    }

    // Parse choices
    if let Some(choices) = json.get("choices").and_then(|v| v.as_array())
        && let Some(choice) = choices.first() {
            let message = match choice.get("message") {
                Some(m) => m,
                None => {
                    output.stop_reason = ImagesStopReason::Error;
                    output.error_message = Some("No message in response".to_string());
                    return Ok(output);
                }
            };

            // Extract text content
            if let Some(content) = message.get("content").and_then(|c| c.as_str())
                && !content.is_empty() {
                    output
                        .output
                        .push(ImagesOutputContent::Text(ImagesTextContent {
                            text: content.to_string(),
                        }));
                }

            // Extract images from the OpenRouter-specific images field
            if let Some(images) = message.get("images").and_then(|v| v.as_array()) {
                for image_val in images {
                    let image_url = image_val.get("image_url").and_then(|iu| {
                        if let Some(s) = iu.as_str() {
                            Some(s.to_string())
                        } else {
                            iu.get("url")
                                .and_then(|u| u.as_str())
                                .map(|s| s.to_string())
                        }
                    });

                    if let Some(url) = image_url
                        && url.starts_with("data:")
                            && let Some((mime_type, data)) = parse_data_url(&url) {
                                output.output.push(ImagesOutputContent::Image(
                                    ImagesImageContent { mime_type, data },
                                ));
                            }
                }
            }
        }

    Ok(output)
}

fn parse_data_url(url: &str) -> Option<(String, String)> {
    // Format: data:mime/type;base64,data
    let after_prefix = url.strip_prefix("data:")?;
    let comma_pos = after_prefix.find(',')?;
    let mime_type = after_prefix[..comma_pos].split(';').next()?.to_string();
    let data = after_prefix[comma_pos + 1..].to_string();
    Some((mime_type, data))
}

fn parse_usage(raw: &serde_json::Value, model: &ImagesModel) -> crate::types::Usage {
    let prompt_tokens = raw
        .get("prompt_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let completion_tokens = raw
        .get("completion_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let prompt_details = raw.get("prompt_tokens_details");
    let reported_cached = prompt_details
        .and_then(|d| d.get("cached_tokens").and_then(|v| v.as_u64()))
        .unwrap_or(0);
    let cache_write = prompt_details
        .and_then(|d| d.get("cache_write_tokens").and_then(|v| v.as_u64()))
        .unwrap_or(0);

    let cache_read = if cache_write > 0 {
        reported_cached.saturating_sub(cache_write)
    } else {
        reported_cached
    };

    let input = prompt_tokens.saturating_sub(cache_read + cache_write);

    let cost_input = (model.cost.input / 1_000_000.0) * input as f64;
    let cost_output = (model.cost.output / 1_000_000.0) * completion_tokens as f64;
    let cost_cache_read = (model.cost.cache_read / 1_000_000.0) * cache_read as f64;
    let cost_cache_write = (model.cost.cache_write / 1_000_000.0) * cache_write as f64;
    let cost_total = cost_input + cost_output + cost_cache_read + cost_cache_write;

    crate::types::Usage {
        input,
        output: completion_tokens,
        cache_read,
        cache_write,
        total_tokens: input + completion_tokens + cache_read + cache_write,
        cost: crate::types::CostBreakdown {
            input: cost_input,
            output: cost_output,
            cache_read: cost_cache_read,
            cache_write: cost_cache_write,
            total: cost_total,
        },
    }
}

/// Register the OpenRouter image provider in the global registry.
pub fn register() {
    crate::images::register_images_api_provider(crate::images::ImagesApiProvider {
        api: KNOWN_IMAGES_API_OPENROUTER.to_string(),
        generate_images: Box::new(|model, context, options| {
            Box::pin(generate_images_openrouter(model, context, options))
        }),
    });
}
