//! Context overflow detection for AI providers

use crate::types::AssistantMessage;

/// Regex patterns to detect context overflow errors from different providers.
const OVERFLOW_PATTERNS: &[&str] = &[
    r"prompt is too long",                    // Anthropic token overflow
    r"request_too_large",                     // Anthropic request byte-size overflow (HTTP 413)
    r"input is too long for requested model", // Amazon Bedrock
    r"exceeds the context window",            // OpenAI (Completions & Responses API)
    r"exceeds (?:the )?(?:model'?s )?maximum context length of [\d,]+ tokens?", // OpenAI-compatible proxies
    r"input token count.*exceeds the maximum",                                  // Google (Gemini)
    r"maximum prompt length is \d+",                                            // xAI (Grok)
    r"reduce the length of the messages",                                       // Groq
    r"maximum context length is \d+ tokens",                                    // OpenRouter
    r"input \(\d+ tokens\) is longer than the model'?s context length \(\d+ tokens\)", // Together AI
    r"exceeds the limit of \d+",           // GitHub Copilot
    r"exceeds the available context size", // llama.cpp
    r"greater than the context length",    // LM Studio
    r"context window exceeds limit",       // MiniMax
    r"exceeded model token limit",         // Kimi For Coding
    r"too large for model with \d+ maximum context length", // Mistral
    r"model_context_window_exceeded",      // z.ai
    r"prompt too long; exceeded (?:max )?context length", // Ollama
    r"context[_ ]length[_ ]exceeded",      // Generic fallback
    r"too many tokens",                    // Generic fallback
    r"token limit exceeded",               // Generic fallback
    r"^4(?:00|13)\s*(?:status code)?\s*\(no body\)", // Cerebras
];

/// Patterns that indicate non-overflow errors (e.g. rate limiting, server errors).
const NON_OVERFLOW_PATTERNS: &[&str] = &[
    r"^(Throttling error|Service unavailable):", // AWS Bedrock
    r"rate limit",                               // Generic rate limiting
    r"too many requests",                        // Generic HTTP 429 style
];

/// Check if a string matches any of the given regex patterns.
fn matches_any(s: &str, patterns: &[&str]) -> bool {
    patterns
        .iter()
        .any(|p| regex::Regex::new(p).is_ok_and(|re| re.is_match(s)))
}

/// Check if an assistant message represents a context overflow error.
///
/// Handles two cases:
/// 1. Error-based overflow: Most providers return stop_reason "error" with a
///    specific error message pattern.
/// 2. Silent overflow: Some providers accept overflow requests and return
///    successfully. For these, we check if usage.input exceeds the context window.
///
/// Pass `context_window` to detect silent overflow (z.ai style) and
/// length-stop overflow (Xiaomi MiMo style).
pub fn is_context_overflow(message: &AssistantMessage, context_window: Option<u64>) -> bool {
    // Case 1: Check error message patterns
    if message.stop_reason == crate::types::StopReason::Error
        && let Some(ref err_msg) = message.error_message {
            let is_non_overflow = matches_any(err_msg, NON_OVERFLOW_PATTERNS);
            if !is_non_overflow && matches_any(err_msg, OVERFLOW_PATTERNS) {
                return true;
            }
        }

    // Case 2: Silent overflow (z.ai style) - successful but usage exceeds context
    if let Some(cw) = context_window {
        if message.stop_reason == crate::types::StopReason::Stop {
            let input_tokens = message.usage.input + message.usage.cache_read;
            if input_tokens > cw {
                return true;
            }
        }

        // Case 3: Length-stop overflow (Xiaomi MiMo style)
        if message.stop_reason == crate::types::StopReason::Length && message.usage.output == 0 {
            let input_tokens = message.usage.input + message.usage.cache_read;
            if input_tokens >= (cw as f64 * 0.99) as u64 {
                return true;
            }
        }
    }

    false
}

/// Get the overflow patterns (for testing).
pub fn get_overflow_patterns() -> Vec<&'static str> {
    OVERFLOW_PATTERNS.to_vec()
}
