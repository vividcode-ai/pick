//! Simple stream options helpers

use std::collections::HashMap;

use crate::types::StreamOptions;

/// Build base StreamOptions from simple options
pub fn build_base_options(
    temperature: Option<f64>,
    max_tokens: Option<u64>,
    api_key: Option<String>,
    headers: Option<std::collections::HashMap<String, String>>,
) -> StreamOptions {
    StreamOptions {
        temperature,
        max_tokens,
        api_key,
        headers,
        ..Default::default()
    }
}

/// Clamp "xhigh" thinking effort to "high"
pub fn clamp_reasoning(effort: Option<&str>) -> Option<&str> {
    match effort {
        Some("xhigh") => Some("high"),
        other => other,
    }
}

/// Adjust max_tokens to leave room for thinking budget.
/// base_max_tokens = None means no explicit caller cap (use model cap).
pub fn adjust_max_tokens_for_thinking(
    base_max_tokens: Option<u64>,
    model_max_tokens: u64,
    reasoning_level: &str,
    custom_budgets: Option<&HashMap<String, u64>>,
) -> (u64, u64) {
    let default_budgets: HashMap<&str, u64> = [
        ("minimal", 1024),
        ("low", 2048),
        ("medium", 8192),
        ("high", 16384),
    ].iter().cloned().collect();

    let min_output_tokens: u64 = 1024;
    let level = clamp_reasoning(Some(reasoning_level)).unwrap_or("medium");

    let budget_value = custom_budgets
        .and_then(|b| b.get(level).copied())
        .or_else(|| default_budgets.get(level).copied())
        .unwrap_or(8192);

    let mut thinking_budget = budget_value;
    let max_tokens = match base_max_tokens {
        Some(base) => std::cmp::min(base + thinking_budget, model_max_tokens),
        None => model_max_tokens,
    };

    if max_tokens <= thinking_budget {
        thinking_budget = max_tokens.saturating_sub(min_output_tokens);
    }

    (max_tokens, thinking_budget)
}
