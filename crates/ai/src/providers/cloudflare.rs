//! Cloudflare AI Gateway / Workers AI utilities

use crate::types::Provider;

/// Workers AI direct endpoint URL template
pub const CLOUDFLARE_WORKERS_AI_BASE_URL: &str =
    "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1";

/// AI Gateway Unified API URL template
pub const CLOUDFLARE_AI_GATEWAY_COMPAT_BASE_URL: &str =
    "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/compat";

/// AI Gateway -> OpenAI passthrough URL template
pub const CLOUDFLARE_AI_GATEWAY_OPENAI_BASE_URL: &str =
    "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai";

/// AI Gateway -> Anthropic passthrough URL template
pub const CLOUDFLARE_AI_GATEWAY_ANTHROPIC_BASE_URL: &str =
    "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic";

/// Check if a provider is a Cloudflare provider (Workers AI or AI Gateway)
pub fn is_cloudflare_provider(provider: &Provider) -> bool {
    is_cloudflare_workers_ai(provider) || is_cloudflare_ai_gateway(provider)
}

/// Check if provider is specifically Workers AI
pub fn is_cloudflare_workers_ai(provider: &Provider) -> bool {
    matches!(provider, Provider::Known(crate::types::KnownProvider::CloudflareWorkersAi))
}

/// Check if provider is specifically AI Gateway
pub fn is_cloudflare_ai_gateway(provider: &Provider) -> bool {
    matches!(provider, Provider::Custom(s) if s == "cloudflare-ai-gateway")
}

/// Resolve a Cloudflare base URL by substituting `{VAR}` placeholders from env vars.
///
/// Example: `https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai`
/// becomes `https://gateway.ai.cloudflare.com/v1/abc123/def456/openai`
pub fn resolve_cloudflare_base_url(base_url: &str) -> Result<String, String> {
    if !base_url.contains('{') {
        return Ok(base_url.to_string());
    }

    let mut result = String::new();
    let mut rest = base_url;

    while let Some(start) = rest.find('{') {
        result.push_str(&rest[..start]);
        rest = &rest[start..];

        let end = rest.find('}')
            .ok_or_else(|| format!("Unmatched {{ in Cloudflare base URL: {}", base_url))?;

        let var_name = &rest[1..end];
        let value = std::env::var(var_name)
            .map_err(|_| format!("{} is required for Cloudflare provider but is not set.", var_name))?;

        result.push_str(&value);
        rest = &rest[end + 1..];
    }

    result.push_str(rest);
    Ok(result)
}

/// Build the appropriate authorization headers for a Cloudflare provider.
/// AI Gateway uses `cf-aig-authorization` instead of `Authorization`.
pub fn get_cloudflare_headers(provider: &Provider, api_key: &str) -> Vec<(String, String)> {
    if is_cloudflare_ai_gateway(provider) {
        vec![
            ("cf-aig-authorization".to_string(), format!("Bearer {}", api_key)),
            ("Authorization".to_string(), String::new()),
        ]
    } else {
        vec![
            ("Authorization".to_string(), format!("Bearer {}", api_key)),
        ]
    }
}
