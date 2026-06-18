//! Model and provider type definitions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Known API types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum KnownApi {
    #[serde(rename = "openai-completions")]
    OpenaiCompletions,
    #[serde(rename = "mistral-conversations")]
    MistralConversations,
    #[serde(rename = "openai-responses")]
    OpenaiResponses,
    #[serde(rename = "azure-openai-responses")]
    AzureOpenaiResponses,
    #[serde(rename = "openai-codex-responses")]
    OpenaiCodexResponses,
    #[serde(rename = "anthropic-messages")]
    AnthropicMessages,
    #[serde(rename = "bedrock-converse-stream")]
    BedrockConverseStream,
    #[serde(rename = "google-generative-ai")]
    GoogleGenerativeAi,
    #[serde(rename = "google-vertex")]
    GoogleVertex,
}

/// API type (known or custom)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum Api {
    Known(KnownApi),
    Custom(String),
}

impl Api {
    pub fn as_str(&self) -> &str {
        match self {
            Api::Known(k) => match k {
                KnownApi::OpenaiCompletions => "openai-completions",
                KnownApi::MistralConversations => "mistral-conversations",
                KnownApi::OpenaiResponses => "openai-responses",
                KnownApi::AzureOpenaiResponses => "azure-openai-responses",
                KnownApi::OpenaiCodexResponses => "openai-codex-responses",
                KnownApi::AnthropicMessages => "anthropic-messages",
                KnownApi::BedrockConverseStream => "bedrock-converse-stream",
                KnownApi::GoogleGenerativeAi => "google-generative-ai",
                KnownApi::GoogleVertex => "google-vertex",
            },
            Api::Custom(s) => s.as_str(),
        }
    }
}

impl From<KnownApi> for Api {
    fn from(api: KnownApi) -> Self {
        Api::Known(api)
    }
}

impl From<String> for Api {
    fn from(s: String) -> Self {
        match s.as_str() {
            "openai-completions" => Api::Known(KnownApi::OpenaiCompletions),
            "mistral-conversations" => Api::Known(KnownApi::MistralConversations),
            "openai-responses" => Api::Known(KnownApi::OpenaiResponses),
            "azure-openai-responses" => Api::Known(KnownApi::AzureOpenaiResponses),
            "openai-codex-responses" => Api::Known(KnownApi::OpenaiCodexResponses),
            "anthropic-messages" => Api::Known(KnownApi::AnthropicMessages),
            "bedrock-converse-stream" => Api::Known(KnownApi::BedrockConverseStream),
            "google-generative-ai" => Api::Known(KnownApi::GoogleGenerativeAi),
            "google-vertex" => Api::Known(KnownApi::GoogleVertex),
            _ => Api::Custom(s),
        }
    }
}

/// Known provider names
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum KnownProvider {
    #[serde(rename = "amazon-bedrock")]
    AmazonBedrock,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "google")]
    Google,
    #[serde(rename = "google-vertex")]
    GoogleVertex,
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "azure-openai-responses")]
    AzureOpenaiResponses,
    #[serde(rename = "openai-codex")]
    OpenaiCodex,
    #[serde(rename = "deepseek")]
    Deepseek,
    #[serde(rename = "github-copilot")]
    GithubCopilot,
    #[serde(rename = "xai")]
    Xai,
    #[serde(rename = "groq")]
    Groq,
    #[serde(rename = "cerebras")]
    Cerebras,
    #[serde(rename = "openrouter")]
    Openrouter,
    #[serde(rename = "mistral")]
    Mistral,
    #[serde(rename = "fireworks")]
    Fireworks,
    #[serde(rename = "together")]
    Together,
    #[serde(rename = "cloudflare-workers-ai")]
    CloudflareWorkersAi,
    #[serde(rename = "cloudflare-ai-gateway")]
    CloudflareAiGateway,
    #[serde(rename = "huggingface")]
    Huggingface,
    #[serde(rename = "kimi-coding")]
    KimiCoding,
    #[serde(rename = "minimax")]
    MiniMax,
    #[serde(rename = "minimax-cn")]
    MiniMaxCn,
    #[serde(rename = "moonshotai")]
    MoonshotAi,
    #[serde(rename = "moonshotai-cn")]
    MoonshotAiCn,
    #[serde(rename = "opencode")]
    OpenCode,
    #[serde(rename = "opencode-go")]
    OpenCodeGo,
    #[serde(rename = "vercel-ai-gateway")]
    VercelAiGateway,
    #[serde(rename = "xiaomi")]
    Xiaomi,
    #[serde(rename = "xiaomi-token-plan-ams")]
    XiaomiTokenPlanAms,
    #[serde(rename = "xiaomi-token-plan-cn")]
    XiaomiTokenPlanCn,
    #[serde(rename = "xiaomi-token-plan-sgp")]
    XiaomiTokenPlanSgp,
    #[serde(rename = "zai")]
    Zai,
}

/// Provider type (known or custom)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum Provider {
    Known(KnownProvider),
    Custom(String),
}

impl Provider {
    pub fn as_str(&self) -> &str {
        match self {
            Provider::Known(k) => match k {
                KnownProvider::AmazonBedrock => "amazon-bedrock",
                KnownProvider::Anthropic => "anthropic",
                KnownProvider::Google => "google",
                KnownProvider::GoogleVertex => "google-vertex",
                KnownProvider::OpenAI => "openai",
                KnownProvider::AzureOpenaiResponses => "azure-openai-responses",
                KnownProvider::OpenaiCodex => "openai-codex",
                KnownProvider::Deepseek => "deepseek",
                KnownProvider::GithubCopilot => "github-copilot",
                KnownProvider::Xai => "xai",
                KnownProvider::Groq => "groq",
                KnownProvider::Cerebras => "cerebras",
                KnownProvider::Openrouter => "openrouter",
                KnownProvider::Mistral => "mistral",
                KnownProvider::Fireworks => "fireworks",
                KnownProvider::Together => "together",
                KnownProvider::CloudflareWorkersAi => "cloudflare-workers-ai",
                KnownProvider::CloudflareAiGateway => "cloudflare-ai-gateway",
                KnownProvider::Huggingface => "huggingface",
                KnownProvider::KimiCoding => "kimi-coding",
                KnownProvider::MiniMax => "minimax",
                KnownProvider::MiniMaxCn => "minimax-cn",
                KnownProvider::MoonshotAi => "moonshotai",
                KnownProvider::MoonshotAiCn => "moonshotai-cn",
                KnownProvider::OpenCode => "opencode",
                KnownProvider::OpenCodeGo => "opencode-go",
                KnownProvider::VercelAiGateway => "vercel-ai-gateway",
                KnownProvider::Xiaomi => "xiaomi",
                KnownProvider::XiaomiTokenPlanAms => "xiaomi-token-plan-ams",
                KnownProvider::XiaomiTokenPlanCn => "xiaomi-token-plan-cn",
                KnownProvider::XiaomiTokenPlanSgp => "xiaomi-token-plan-sgp",
                KnownProvider::Zai => "zai",
            },
            Provider::Custom(s) => s.as_str(),
        }
    }
}

impl From<KnownProvider> for Provider {
    fn from(p: KnownProvider) -> Self {
        Provider::Known(p)
    }
}

impl From<String> for Provider {
    fn from(s: String) -> Self {
        match s.as_str() {
            "amazon-bedrock" => Provider::Known(KnownProvider::AmazonBedrock),
            "anthropic" => Provider::Known(KnownProvider::Anthropic),
            "google" => Provider::Known(KnownProvider::Google),
            "google-vertex" => Provider::Known(KnownProvider::GoogleVertex),
            "openai" => Provider::Known(KnownProvider::OpenAI),
            "deepseek" => Provider::Known(KnownProvider::Deepseek),
            "github-copilot" => Provider::Known(KnownProvider::GithubCopilot),
            "mistral" => Provider::Known(KnownProvider::Mistral),
            "openrouter" => Provider::Known(KnownProvider::Openrouter),
            "together" => Provider::Known(KnownProvider::Together),
            "xai" => Provider::Known(KnownProvider::Xai),
            "groq" => Provider::Known(KnownProvider::Groq),
            "cerebras" => Provider::Known(KnownProvider::Cerebras),
            "fireworks" => Provider::Known(KnownProvider::Fireworks),
            "cloudflare-workers-ai" => Provider::Known(KnownProvider::CloudflareWorkersAi),
            "cloudflare-ai-gateway" => Provider::Known(KnownProvider::CloudflareAiGateway),
            "huggingface" => Provider::Known(KnownProvider::Huggingface),
            "kimi-coding" => Provider::Known(KnownProvider::KimiCoding),
            "minimax" => Provider::Known(KnownProvider::MiniMax),
            "minimax-cn" => Provider::Known(KnownProvider::MiniMaxCn),
            "moonshotai" => Provider::Known(KnownProvider::MoonshotAi),
            "moonshotai-cn" => Provider::Known(KnownProvider::MoonshotAiCn),
            "opencode" => Provider::Known(KnownProvider::OpenCode),
            "opencode-go" => Provider::Known(KnownProvider::OpenCodeGo),
            "vercel-ai-gateway" => Provider::Known(KnownProvider::VercelAiGateway),
            "xiaomi" => Provider::Known(KnownProvider::Xiaomi),
            "xiaomi-token-plan-ams" => Provider::Known(KnownProvider::XiaomiTokenPlanAms),
            "xiaomi-token-plan-cn" => Provider::Known(KnownProvider::XiaomiTokenPlanCn),
            "xiaomi-token-plan-sgp" => Provider::Known(KnownProvider::XiaomiTokenPlanSgp),
            "zai" => Provider::Known(KnownProvider::Zai),
            _ => Provider::Custom(s),
        }
    }
}

/// Thinking/reasoning level
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ThinkingLevel {
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "minimal")]
    Minimal,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "xhigh")]
    XHigh,
}

impl ThinkingLevel {
    pub fn is_enabled(&self) -> bool {
        !matches!(self, ThinkingLevel::Off)
    }
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub total_tokens: u64,
    pub cost: CostBreakdown,
}

impl Usage {
    pub fn zero() -> Self {
        Self {
            input: 0,
            output: 0,
            cache_read: 0,
            cache_write: 0,
            total_tokens: 0,
            cost: CostBreakdown::zero(),
        }
    }
}

/// Cost breakdown for token usage
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CostBreakdown {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    pub total: f64,
}

impl CostBreakdown {
    pub fn zero() -> Self {
        Self {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total: 0.0,
        }
    }
}

/// Compatibility overrides for OpenAI-compatible completions APIs.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenAICompletionsCompat {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_store: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_developer_role: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_reasoning_effort: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_usage_in_streaming: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_tool_result_name: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_assistant_after_tool_result: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_thinking_as_text: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_reasoning_content_on_assistant_messages: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_strict_mode: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_session_affinity_headers: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_long_cache_retention: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zai_tool_stream: Option<bool>,
}

/// Compatibility overrides for OpenAI Responses APIs.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenAIResponsesCompat {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_session_id_header: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_long_cache_retention: Option<bool>,
}

/// Compatibility overrides for Anthropic Messages-compatible APIs.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnthropicMessagesCompat {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_eager_tool_input_streaming: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_long_cache_retention: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_session_affinity_headers: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_cache_control_on_tools: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_adaptive_thinking: Option<bool>,
}

/// Per-API compatibility settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompatConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai_completions: Option<OpenAICompletionsCompat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai_responses: Option<OpenAIResponsesCompat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic_messages: Option<AnthropicMessagesCompat>,
}

/// Model definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub api: Api,
    pub provider: Provider,
    pub base_url: String,
    pub reasoning: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_level_map: Option<HashMap<String, Option<String>>>,
    pub input_capabilities: Vec<Capability>,
    pub cost: ModelCost,
    pub context_window: u64,
    pub max_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    /// Compatibility overrides per API type. If not set, auto-detected from base_url.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compat: Option<CompatConfig>,
}

/// Model input/output capability
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Capability {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "image")]
    Image,
}

/// Model cost per million tokens
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelCost {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}
