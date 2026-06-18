//! Model resolution, scoping, and initial selection


use crate::core::defaults::DEFAULT_THINKING_LEVEL;
use crate::core::model_registry::{Model, ModelRegistry};

// ============================================================================
// Default model per provider
// ============================================================================

pub const DEFAULT_MODEL_PER_PROVIDER: &[(&str, &str)] = &[
    ("anthropic", "claude-opus-4-7"),
    ("openai", "gpt-5.4"),
    ("deepseek", "deepseek-v4-pro"),
    ("google", "gemini-3.1-pro-preview"),
    ("openrouter", "moonshotai/kimi-k2.6"),
];

// ============================================================================
// ScopedModel
// ============================================================================

#[derive(Debug, Clone)]
pub struct ScopedModel {
    pub model: Model,
    pub thinking_level: Option<String>,
}

// ============================================================================
// Model matching
// ============================================================================

/// Helper to check if a model ID looks like an alias (no date suffix)
fn is_alias(id: &str) -> bool {
    if id.ends_with("-latest") {
        return true;
    }
    let date_pattern = regex::Regex::new(r"-\d{8}$").unwrap();
    !date_pattern.is_match(id)
}

/// Find an exact model reference match
pub fn find_exact_model_reference_match<'a>(
    model_reference: &str,
    available_models: &'a [Model],
) -> Option<&'a Model> {
    let trimmed = model_reference.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed.to_lowercase();

    // Canonical: provider/model
    let canonical_matches: Vec<&Model> = available_models.iter()
        .filter(|m| format!("{}/{}", m.provider.to_lowercase(), m.id.to_lowercase()) == normalized)
        .collect();
    if canonical_matches.len() == 1 {
        return Some(canonical_matches[0]);
    }
    if canonical_matches.len() > 1 {
        return None;
    }

    // Try provider/model split
    if let Some(slash_idx) = trimmed.find('/') {
        let provider = trimmed[..slash_idx].trim();
        let model_id = trimmed[slash_idx + 1..].trim();
        if !provider.is_empty() && !model_id.is_empty() {
            let provider_matches: Vec<&Model> = available_models.iter()
                .filter(|m| m.provider.to_lowercase() == provider.to_lowercase()
                    && m.id.to_lowercase() == model_id.to_lowercase())
                .collect();
            if provider_matches.len() == 1 {
                return Some(provider_matches[0]);
            }
        }
    }

    // Bare model ID match
    let id_matches: Vec<&Model> = available_models.iter()
        .filter(|m| m.id.to_lowercase() == normalized)
        .collect();
    if id_matches.len() == 1 {
        return Some(id_matches[0]);
    }
    None
}

/// Try to match a pattern to a model
fn try_match_model<'a>(model_pattern: &str, available_models: &'a [Model]) -> Option<&'a Model> {
    if let Some(exact) = find_exact_model_reference_match(model_pattern, available_models) {
        return Some(exact);
    }

    let lower = model_pattern.to_lowercase();
    let matches: Vec<&Model> = available_models.iter()
        .filter(|m| m.id.to_lowercase().contains(&lower)
            || m.name.to_lowercase().contains(&lower))
        .collect();

    if matches.is_empty() {
        return None;
    }

    let aliases: Vec<&&Model> = matches.iter().filter(|m| is_alias(&m.id)).collect();
    let dated: Vec<&&Model> = matches.iter().filter(|m| !is_alias(&m.id)).collect();

    if !aliases.is_empty() {
        let mut sorted = aliases.clone();
        sorted.sort_by(|a, b| b.id.cmp(&a.id));
        Some(sorted[0])
    } else if !dated.is_empty() {
        let mut sorted = dated.clone();
        sorted.sort_by(|a, b| b.id.cmp(&a.id));
        Some(sorted[0])
    } else {
        None
    }
}

// ============================================================================
// ParsedModelResult
// ============================================================================

pub struct ParsedModelResult<'a> {
    pub model: Option<&'a Model>,
    pub thinking_level: Option<String>,
    pub warning: Option<String>,
}

/// Parse a pattern to extract model and thinking level
pub fn parse_model_pattern<'a>(
    pattern: &str,
    available_models: &'a [Model],
) -> ParsedModelResult<'a> {
    // Try exact match first
    if let Some(exact) = try_match_model(pattern, available_models) {
        return ParsedModelResult {
            model: Some(exact),
            thinking_level: None,
            warning: None,
        };
    }

    // Try splitting on last colon
    let last_colon = pattern.rfind(':');
    let last_colon = match last_colon {
        Some(i) => i,
        None => return ParsedModelResult { model: None, thinking_level: None, warning: None },
    };

    let prefix = &pattern[..last_colon];
    let suffix = &pattern[last_colon + 1..];

    let valid_levels = ["off", "minimal", "low", "medium", "high", "xhigh"];

    if valid_levels.contains(&suffix) {
        let result = parse_model_pattern(prefix, available_models);
        if let Some(model) = result.model {
            let tl = if result.warning.is_none() { Some(suffix.to_string()) } else { None };
            return ParsedModelResult {
                model: Some(model),
                thinking_level: tl,
                warning: result.warning,
            };
        }
        return result;
    }

    // Invalid suffix - recurse with warning
    let result = parse_model_pattern(prefix, available_models);
    if result.model.is_some() {
        return ParsedModelResult {
            model: result.model,
            thinking_level: None,
            warning: Some(format!(
                "Invalid thinking level \"{}\" in pattern \"{}\". Using default instead.",
                suffix, pattern
            )),
        };
    }
    result
}

// ============================================================================
// Resolve model patterns (scoping)
// ============================================================================

/// Resolve model patterns to actual Model objects with optional thinking levels
pub async fn resolve_model_scope(patterns: &[String], model_registry: &ModelRegistry) -> Vec<ScopedModel> {
    let available_models = model_registry.get_available();
    let mut scoped_models: Vec<ScopedModel> = Vec::new();

    for pattern in patterns {
        let mut tl = None;
        let mut glob_pattern = pattern.as_str();

        // Check for thinking level suffix
        if let Some(colon_idx) = pattern.rfind(':') {
            let suffix = &pattern[colon_idx + 1..];
            let valid_levels = ["off", "minimal", "low", "medium", "high", "xhigh"];
            if valid_levels.contains(&suffix) {
                tl = Some(suffix.to_string());
                glob_pattern = &pattern[..colon_idx];
            }
        }

        // Check for glob characters
        if glob_pattern.contains('*') || glob_pattern.contains('?') || glob_pattern.contains('[') {
            let lower_glob = glob_pattern.to_lowercase();
            let matching: Vec<&Model> = available_models.iter()
                .filter(|m| {
                    let full_id = format!("{}/{}", m.provider.to_lowercase(), m.id.to_lowercase());
                    let id_only = m.id.to_lowercase();
                    glob_match(&full_id, &lower_glob) || glob_match(&id_only, &lower_glob)
                })
                .collect();

            if matching.is_empty() {
                eprintln!("Warning: No models match pattern \"{}\"", pattern);
                continue;
            }

            for model in matching {
                if !scoped_models.iter().any(|sm| sm.model.id == model.id && sm.model.provider == model.provider) {
                    scoped_models.push(ScopedModel {
                        model: model.clone(),
                        thinking_level: tl.clone(),
                    });
                }
            }
            continue;
        }

        let result = parse_model_pattern(glob_pattern, &available_models);

        if let Some(ref warning) = result.warning {
            eprintln!("Warning: {}", warning);
        }

        match result.model {
            Some(model) => {
                if !scoped_models.iter().any(|sm| sm.model.id == model.id && sm.model.provider == model.provider) {
                    scoped_models.push(ScopedModel {
                        model: model.clone(),
                        thinking_level: result.thinking_level.or(tl),
                    });
                }
            }
            None => {
                eprintln!("Warning: No models match pattern \"{}\"", pattern);
            }
        }
    }

    scoped_models
}

// ============================================================================
// CLI model resolution
// ============================================================================

pub struct ResolveCliModelResult {
    pub model: Option<Model>,
    pub thinking_level: Option<String>,
    pub warning: Option<String>,
    pub error: Option<String>,
}

/// Resolve a single model from CLI flags
pub fn resolve_cli_model(
    cli_provider: Option<&str>,
    cli_model: Option<&str>,
    model_registry: &ModelRegistry,
) -> ResolveCliModelResult {
    let cli_model = match cli_model {
        Some(m) => m,
        None => return ResolveCliModelResult { model: None, thinking_level: None, warning: None, error: None },
    };

    let available_models = model_registry.get_all();
    if available_models.is_empty() {
        return ResolveCliModelResult {
            model: None,
            thinking_level: None,
            warning: None,
            error: Some("No models available. Check your installation or add models to models.json.".to_string()),
        };
    }

    let provider_map: std::collections::HashMap<String, String> = available_models.iter()
        .map(|m| (m.provider.to_lowercase(), m.provider.clone()))
        .collect();

    let provider = cli_provider
        .and_then(|p| provider_map.get(&p.to_lowercase()).cloned());

    if cli_provider.is_some() && provider.is_none() {
        return ResolveCliModelResult {
            model: None,
            thinking_level: None,
            warning: None,
            error: Some(format!(
                "Unknown provider \"{}\". Use --list-models to see available providers/models.",
                cli_provider.unwrap()
            )),
        };
    }

    let mut pattern = cli_model;
    let mut inferred_provider = false;
    let mut resolved_provider = provider;

    if resolved_provider.is_none() {
        if let Some(slash_idx) = cli_model.find('/') {
            let maybe_provider = &cli_model[..slash_idx];
            if let Some(canonical) = provider_map.get(&maybe_provider.to_lowercase()) {
                resolved_provider = Some(canonical.clone());
                pattern = &cli_model[slash_idx + 1..];
                inferred_provider = true;
            }
        }
    }

    // Try exact match without provider inference
    if resolved_provider.is_none() {
        let lower = cli_model.to_lowercase();
        if let Some(exact) = available_models.iter().find(|m| {
            m.id.to_lowercase() == lower || format!("{}/{}", m.provider.to_lowercase(), m.id.to_lowercase()) == lower
        }) {
            return ResolveCliModelResult {
                model: Some(exact.clone()),
                warning: None,
                thinking_level: None,
                error: None,
            };
        }
    }

    let candidates: Vec<Model> = match &resolved_provider {
        Some(p) => available_models.iter().filter(|m| m.provider == *p).cloned().collect(),
        None => available_models.iter().cloned().collect(),
    };

    let result = parse_model_pattern(pattern, &candidates);

    if let Some(model) = result.model {
        return ResolveCliModelResult {
            model: Some(model.clone()),
            thinking_level: result.thinking_level,
            warning: result.warning,
            error: None,
        };
    }

    // If no match but we inferred a provider, try broader match
    if inferred_provider {
        let lower = cli_model.to_lowercase();
        if let Some(exact) = available_models.iter().find(|m| {
            m.id.to_lowercase() == lower || format!("{}/{}", m.provider.to_lowercase(), m.id.to_lowercase()) == lower
        }) {
            return ResolveCliModelResult {
                model: Some(exact.clone()),
                warning: None,
                thinking_level: None,
                error: None,
            };
        }
    }

    ResolveCliModelResult {
        model: None,
        thinking_level: None,
        warning: None,
        error: Some(format!("Model \"{}\" not found. Use --list-models to see available models.", cli_model)),
    }
}

// ============================================================================
// Find initial model
// ============================================================================

pub struct InitialModelResult {
    pub model: Option<Model>,
    pub thinking_level: String,
    pub fallback_message: Option<String>,
}

/// Find the initial model to use based on priority
pub async fn find_initial_model(
    cli_provider: Option<&str>,
    cli_model: Option<&str>,
    scoped_models: &[ScopedModel],
    is_continuing: bool,
    default_provider: Option<&str>,
    default_model_id: Option<&str>,
    default_thinking_level: Option<&str>,
    model_registry: &ModelRegistry,
) -> InitialModelResult {
    let tl = default_thinking_level.unwrap_or(DEFAULT_THINKING_LEVEL);

    // 1. CLI args take priority
    if cli_provider.is_some() && cli_model.is_some() {
        let resolved = resolve_cli_model(cli_provider, cli_model, model_registry);
        if let Some(ref error) = resolved.error {
            eprintln!("{}", error);
            std::process::exit(1);
        }
        if let Some(model) = resolved.model {
            return InitialModelResult {
                model: Some(model),
                thinking_level: DEFAULT_THINKING_LEVEL.to_string(),
                fallback_message: None,
            };
        }
    }

    // 2. Use first model from scoped models
    if !scoped_models.is_empty() && !is_continuing {
        return InitialModelResult {
            model: Some(scoped_models[0].model.clone()),
            thinking_level: scoped_models[0].thinking_level.clone().unwrap_or_else(|| tl.to_string()),
            fallback_message: None,
        };
    }

    // 3. Try saved default from settings
    if let (Some(provider), Some(model_id)) = (default_provider, default_model_id) {
        if let Some(found) = model_registry.find(provider, model_id) {
            return InitialModelResult {
                model: Some(found),
                thinking_level: tl.to_string(),
                fallback_message: None,
            };
        }
    }

    // 4. Try first available model with valid API key
    let available_models = model_registry.get_available();
    if !available_models.is_empty() {
        for (provider, default_id) in DEFAULT_MODEL_PER_PROVIDER {
            if let Some(m) = available_models.iter().find(|m| m.provider == *provider && m.id == *default_id) {
                return InitialModelResult {
                    model: Some(m.clone()),
                    thinking_level: DEFAULT_THINKING_LEVEL.to_string(),
                    fallback_message: None,
                };
            }
        }
        return InitialModelResult {
            model: Some(available_models[0].clone()),
            thinking_level: DEFAULT_THINKING_LEVEL.to_string(),
            fallback_message: None,
        };
    }

    InitialModelResult {
        model: None,
        thinking_level: DEFAULT_THINKING_LEVEL.to_string(),
        fallback_message: None,
    }
}

// ============================================================================
// Simple glob matching
// ============================================================================

fn glob_match(text: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') && !pattern.contains('?') {
        return text == pattern;
    }
    let re_str = format!("^{}$", regex::escape(pattern)
        .replace("\\*", ".*")
        .replace("\\?", "."));
    regex::Regex::new(&re_str).map(|re| re.is_match(text)).unwrap_or(false)
}
