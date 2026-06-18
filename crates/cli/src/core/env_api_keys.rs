//! Provider-to-env-var API key mapping.


use std::collections::HashMap;
use std::sync::LazyLock;

static PROVIDER_ENV_MAP: LazyLock<HashMap<&'static str, &'static [&'static str]>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("github-copilot", &["COPILOT_GITHUB_TOKEN"] as &[&'static str]);
    m.insert("anthropic", &["ANTHROPIC_OAUTH_TOKEN", "ANTHROPIC_API_KEY"]);
    m.insert("openai", &["OPENAI_API_KEY"]);
    m.insert("azure-openai-responses", &["AZURE_OPENAI_API_KEY"]);
    m.insert("deepseek", &["DEEPSEEK_API_KEY"]);
    m.insert("google", &["GEMINI_API_KEY"]);
    m.insert("google-vertex", &["GOOGLE_CLOUD_API_KEY"]);
    m.insert("groq", &["GROQ_API_KEY"]);
    m.insert("cerebras", &["CEREBRAS_API_KEY"]);
    m.insert("xai", &["XAI_API_KEY"]);
    m.insert("openrouter", &["OPENROUTER_API_KEY"]);
    m.insert("vercel-ai-gateway", &["AI_GATEWAY_API_KEY"]);
    m.insert("zai", &["ZAI_API_KEY"]);
    m.insert("mistral", &["MISTRAL_API_KEY"]);
    m.insert("minimax", &["MINIMAX_API_KEY"]);
    m.insert("minimax-cn", &["MINIMAX_CN_API_KEY"]);
    m.insert("moonshotai", &["MOONSHOTAI_API_KEY"]);
    m.insert("huggingface", &["HF_TOKEN"]);
    m.insert("fireworks", &["FIREWORKS_API_KEY"]);
    m.insert("together", &["TOGETHER_API_KEY"]);
    m.insert("opencode", &["OPENCODE_API_KEY"]);
    m.insert("opencode-go", &["OPENCODE_API_KEY"]);
    m.insert("kimi-coding", &["KIMI_API_KEY"]);
    m.insert("cloudflare-workers-ai", &["CLOUDFLARE_API_KEY"]);
    m.insert("cloudflare-ai-gateway", &["CLOUDFLARE_API_KEY"]);
    m.insert("xiaomi", &["XIAOMI_API_KEY"]);
    m
});

/// Find which env vars are set for a given provider.
/// Returns a list of env var names that have values set.
pub fn find_env_keys(provider: &str) -> Option<Vec<String>> {
    let env_vars = PROVIDER_ENV_MAP.get(provider)?;
    let found: Vec<String> = env_vars
        .iter()
        .filter(|&&var| std::env::var(var).is_ok())
        .map(|&s| s.to_string())
        .collect();
    if found.is_empty() { None } else { Some(found) }
}

/// Get the API key for a provider from its configured environment variable.
pub fn get_env_api_key(provider: &str) -> Option<String> {
    // Check standard mapped env vars
    if let Some(env_vars) = PROVIDER_ENV_MAP.get(provider) {
        for &var in env_vars.iter() {
            if let Ok(val) = std::env::var(var) {
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }

    // Special cases: providers authenticated via ambient credentials
    match provider {
        "google-vertex" => {
            let has_creds = std::env::var("GOOGLE_APPLICATION_CREDENTIALS").is_ok();
            let has_project = std::env::var("GOOGLE_CLOUD_PROJECT").is_ok()
                || std::env::var("GCLOUD_PROJECT").is_ok();
            let has_location = std::env::var("GOOGLE_CLOUD_LOCATION").is_ok();
            if has_creds && has_project && has_location {
                return Some("<authenticated>".to_string());
            }
        }
        "amazon-bedrock" => {
            let has_profile = std::env::var("AWS_PROFILE").is_ok();
            let has_keys = std::env::var("AWS_ACCESS_KEY_ID").is_ok()
                && std::env::var("AWS_SECRET_ACCESS_KEY").is_ok();
            let has_token = std::env::var("AWS_BEARER_TOKEN_BEDROCK").is_ok();
            let has_container = std::env::var("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI").is_ok()
                || std::env::var("AWS_CONTAINER_CREDENTIALS_FULL_URI").is_ok();
            let has_web_token = std::env::var("AWS_WEB_IDENTITY_TOKEN_FILE").is_ok();
            if has_profile || has_keys || has_token || has_container || has_web_token {
                return Some("<authenticated>".to_string());
            }
        }
        _ => {}
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_providers() {
        assert!(PROVIDER_ENV_MAP.contains_key("openai"));
        assert!(PROVIDER_ENV_MAP.contains_key("anthropic"));
        assert!(PROVIDER_ENV_MAP.contains_key("google"));
        assert!(PROVIDER_ENV_MAP.contains_key("openrouter"));
    }

    #[test]
    fn test_anthropic_precedence() {
        let vars = PROVIDER_ENV_MAP.get("anthropic").unwrap();
        assert_eq!(vars[0], "ANTHROPIC_OAUTH_TOKEN");
        assert_eq!(vars[1], "ANTHROPIC_API_KEY");
    }
}
