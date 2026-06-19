//! Provider-to-env-var API key mapping.

use std::collections::HashMap;
use std::sync::LazyLock;

static PROVIDER_ENV_MAP: LazyLock<HashMap<&'static str, &'static [&'static str]>> =
    LazyLock::new(|| {
        let mut m = HashMap::new();
        m.insert(
            "github-copilot",
            &["COPILOT_GITHUB_TOKEN"] as &[&'static str],
        );
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

/// Get the API key for a provider from its configured environment variable.
/// Matches the env vars mapped to each provider.
pub fn get_env_api_key(provider: &str) -> Option<String> {
    if let Some(env_vars) = PROVIDER_ENV_MAP.get(provider) {
        for &var in env_vars.iter() {
            if let Ok(val) = std::env::var(var)
                && !val.is_empty() {
                    return Some(val);
                }
        }
    }
    None
}
