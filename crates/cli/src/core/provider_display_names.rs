//! Provider display names - human-readable names for provider IDs


use std::collections::HashMap;

/// Get the built-in provider display names
fn built_in_provider_display_names() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("anthropic", "Anthropic");
    m.insert("amazon-bedrock", "Amazon Bedrock");
    m.insert("azure-openai-responses", "Azure OpenAI Responses");
    m.insert("cerebras", "Cerebras");
    m.insert("cloudflare-ai-gateway", "Cloudflare AI Gateway");
    m.insert("cloudflare-workers-ai", "Cloudflare Workers AI");
    m.insert("deepseek", "DeepSeek");
    m.insert("fireworks", "Fireworks");
    m.insert("google", "Google Gemini");
    m.insert("google-vertex", "Google Vertex AI");
    m.insert("groq", "Groq");
    m.insert("huggingface", "Hugging Face");
    m.insert("kimi-coding", "Kimi For Coding");
    m.insert("mistral", "Mistral");
    m.insert("minimax", "MiniMax");
    m.insert("minimax-cn", "MiniMax (China)");
    m.insert("moonshotai", "Moonshot AI");
    m.insert("moonshotai-cn", "Moonshot AI (China)");
    m.insert("opencode", "OpenCode Zen");
    m.insert("opencode-go", "OpenCode Go");
    m.insert("openai", "OpenAI");
    m.insert("openrouter", "OpenRouter");
    m.insert("together", "Together AI");
    m.insert("vercel-ai-gateway", "Vercel AI Gateway");
    m.insert("xai", "xAI");
    m.insert("zai", "ZAI");
    m.insert("xiaomi", "Xiaomi MiMo");
    m.insert("xiaomi-token-plan-cn", "Xiaomi MiMo Token Plan (China)");
    m.insert("xiaomi-token-plan-ams", "Xiaomi MiMo Token Plan (Amsterdam)");
    m.insert("xiaomi-token-plan-sgp", "Xiaomi MiMo Token Plan (Singapore)");
    m
}

/// Get the display name for a provider ID
pub fn get_provider_display_name(provider: &str) -> &str {
    let names = built_in_provider_display_names();
    names.get(provider).copied().unwrap_or(provider)
}
