//! Model registry - manages built-in and custom models, provides API key resolution

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::config::get_agent_dir;
use crate::core::auth_storage::{AuthStatus, AuthStorage};
use crate::core::resolve_config_value::{resolve_config_value_or_throw, resolve_headers_or_throw};

// ============================================================================
// Model types
// ============================================================================

#[derive(Debug, Clone)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub api: String,
    pub provider: String,
    pub base_url: String,
    pub reasoning: bool,
    pub input: Vec<String>,
    pub cost: ModelCost,
    pub context_window: u64,
    pub max_tokens: u64,
}

#[derive(Debug, Clone)]
pub struct ModelCost {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

impl Default for ModelCost {
    fn default() -> Self {
        Self {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        }
    }
}

// ============================================================================
// Models.json types
// ============================================================================

#[derive(Debug, Clone, serde::Deserialize)]
struct RawModelsConfig {
    #[serde(default)]
    providers: HashMap<String, RawProviderConfig>,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct RawProviderConfig {
    name: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
    api: Option<String>,
    headers: Option<HashMap<String, String>>,
    compat: Option<serde_json::Value>,
    auth_header: Option<bool>,
    models: Option<Vec<RawModelDefinition>>,
    model_overrides: Option<HashMap<String, RawModelOverride>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RawModelDefinition {
    id: String,
    name: Option<String>,
    api: Option<String>,
    base_url: Option<String>,
    reasoning: Option<bool>,
    input: Option<Vec<String>>,
    cost: Option<RawModelCost>,
    context_window: Option<u64>,
    max_tokens: Option<u64>,
    headers: Option<HashMap<String, String>>,
    compat: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct RawModelCost {
    input: Option<f64>,
    output: Option<f64>,
    cache_read: Option<f64>,
    cache_write: Option<f64>,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct RawModelOverride {
    name: Option<String>,
    reasoning: Option<bool>,
    input: Option<Vec<String>>,
    cost: Option<RawModelCost>,
    context_window: Option<u64>,
    max_tokens: Option<u64>,
    headers: Option<HashMap<String, String>>,
    compat: Option<serde_json::Value>,
}

// ============================================================================
// Provider request config
// ============================================================================

#[derive(Debug, Clone, Default)]
struct ProviderRequestConfig {
    api_key: Option<String>,
    headers: Option<HashMap<String, String>>,
    auth_header: Option<bool>,
}

// ============================================================================
// ResolvedRequestAuth
// ============================================================================

#[derive(Debug, Clone)]
pub struct ResolvedRequestAuth {
    pub ok: bool,
    pub api_key: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub error: Option<String>,
}

// ============================================================================
// ModelRegistry
// ============================================================================

pub struct ModelRegistry {
    models: Mutex<Vec<Model>>,
    provider_request_configs: Mutex<HashMap<String, ProviderRequestConfig>>,
    model_request_headers: Mutex<HashMap<String, HashMap<String, String>>>,
    load_error: Mutex<Option<String>>,
    pub auth_storage: AuthStorage,
    models_json_path: Option<PathBuf>,
}

impl ModelRegistry {
    pub fn create(auth_storage: AuthStorage, models_json_path: Option<PathBuf>) -> Self {
        let path = models_json_path.unwrap_or_else(|| get_agent_dir().join("models.json"));
        let instance = Self {
            models: Mutex::new(Vec::new()),
            provider_request_configs: Mutex::new(HashMap::new()),
            model_request_headers: Mutex::new(HashMap::new()),
            load_error: Mutex::new(None),
            auth_storage,
            models_json_path: Some(path),
        };
        instance.load_models();
        instance
    }

    pub fn in_memory(auth_storage: AuthStorage) -> Self {
        Self::create(auth_storage, None)
    }

    pub fn refresh(&self) {
        self.provider_request_configs.lock().unwrap().clear();
        self.model_request_headers.lock().unwrap().clear();
        *self.load_error.lock().unwrap() = None;
        self.load_models();
    }

    pub fn get_error(&self) -> Option<String> {
        self.load_error.lock().unwrap().clone()
    }

    fn load_models(&self) {
        let custom_result = self
            .models_json_path
            .as_ref()
            .and_then(|p| self.load_custom_models(p));

        if let Some(ref result) = custom_result {
            if let Some(ref error) = result.error {
                *self.load_error.lock().unwrap() = Some(error.clone());
            }
        }

        let mut combined = self.load_builtin_models();

        if let Some(ref result) = custom_result {
            for custom_model in &result.models {
                let existing = combined.iter_mut().find(|m: &&mut Model| {
                    m.provider == custom_model.provider && m.id == custom_model.id
                });
                match existing {
                    Some(ex) => *ex = custom_model.clone(),
                    None => combined.push(custom_model.clone()),
                }
            }
        }

        *self.models.lock().unwrap() = combined;
    }

    fn load_builtin_models(&self) -> Vec<Model> {
        vec![
            Model {
                id: "claude-opus-4-7".to_string(),
                name: "Claude Opus 4.7".to_string(),
                api: "anthropic".to_string(),
                provider: "anthropic".to_string(),
                base_url: "https://api.anthropic.com".to_string(),
                reasoning: true,
                input: vec!["text".to_string(), "image".to_string()],
                cost: ModelCost {
                    input: 15.0,
                    output: 75.0,
                    cache_read: 1.5,
                    cache_write: 7.5,
                },
                context_window: 200000,
                max_tokens: 8192,
            },
            Model {
                id: "claude-sonnet-4-6".to_string(),
                name: "Claude Sonnet 4.6".to_string(),
                api: "anthropic".to_string(),
                provider: "anthropic".to_string(),
                base_url: "https://api.anthropic.com".to_string(),
                reasoning: true,
                input: vec!["text".to_string(), "image".to_string()],
                cost: ModelCost {
                    input: 3.0,
                    output: 15.0,
                    cache_read: 0.3,
                    cache_write: 1.5,
                },
                context_window: 200000,
                max_tokens: 8192,
            },
        ]
    }

    fn load_custom_models(&self, models_json_path: &Path) -> Option<CustomModelsResult> {
        if !models_json_path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(models_json_path).ok()?;
        let stripped = strip_json_comments(&content);
        let parsed: RawModelsConfig = serde_json::from_str(&stripped).ok()?;

        let mut models = Vec::new();
        let mut model_request_headers = self.model_request_headers.lock().unwrap();

        for (provider_name, provider_config) in &parsed.providers {
            self.store_provider_request_config(provider_name, &provider_config);

            if let Some(ref model_defs) = provider_config.models {
                for model_def in model_defs {
                    let api = model_def
                        .api
                        .as_ref()
                        .or(provider_config.api.as_ref())
                        .map(|s| s.clone())
                        .unwrap_or_else(|| "openai-completions".to_string());

                    let base_url = model_def
                        .base_url
                        .as_ref()
                        .or(provider_config.base_url.as_ref())
                        .map(|s| s.clone())
                        .unwrap_or_default();

                    let raw_cost = model_def.cost.as_ref().or(provider_config.cost());

                    models.push(Model {
                        id: model_def.id.clone(),
                        name: model_def
                            .name
                            .clone()
                            .unwrap_or_else(|| model_def.id.clone()),
                        api,
                        provider: provider_name.clone(),
                        base_url,
                        reasoning: model_def.reasoning.unwrap_or(false),
                        input: model_def
                            .input
                            .clone()
                            .unwrap_or_else(|| vec!["text".to_string()]),
                        cost: ModelCost {
                            input: raw_cost.and_then(|c| c.input).unwrap_or(0.0),
                            output: raw_cost.and_then(|c| c.output).unwrap_or(0.0),
                            cache_read: raw_cost.and_then(|c| c.cache_read).unwrap_or(0.0),
                            cache_write: raw_cost.and_then(|c| c.cache_write).unwrap_or(0.0),
                        },
                        context_window: model_def.context_window.unwrap_or(128000),
                        max_tokens: model_def.max_tokens.unwrap_or(16384),
                    });

                    if let Some(ref headers) = model_def.headers {
                        let key = format!("{}:{}", provider_name, model_def.id);
                        model_request_headers.insert(key, headers.clone());
                    }
                }
            }
        }

        Some(CustomModelsResult {
            models,
            error: None,
        })
    }

    fn store_provider_request_config(&self, provider_name: &str, config: &RawProviderConfig) {
        if config.api_key.is_none() && config.headers.is_none() && config.auth_header.is_none() {
            return;
        }
        self.provider_request_configs.lock().unwrap().insert(
            provider_name.to_string(),
            ProviderRequestConfig {
                api_key: config.api_key.clone(),
                headers: config.headers.clone(),
                auth_header: config.auth_header,
            },
        );
    }

    // ========================================================================
    // Public API
    // ========================================================================

    pub fn get_all(&self) -> Vec<Model> {
        self.models.lock().unwrap().clone()
    }

    pub fn get_available(&self) -> Vec<Model> {
        self.models
            .lock()
            .unwrap()
            .iter()
            .filter(|m| self.has_configured_auth(m))
            .cloned()
            .collect()
    }

    pub fn find(&self, provider: &str, model_id: &str) -> Option<Model> {
        self.models
            .lock()
            .unwrap()
            .iter()
            .find(|m| m.provider == provider && m.id == model_id)
            .cloned()
    }

    pub fn has_configured_auth(&self, model: &Model) -> bool {
        if self.auth_storage.has_auth(&model.provider) {
            return true;
        }
        self.provider_request_configs
            .lock()
            .unwrap()
            .get(&model.provider)
            .and_then(|c| c.api_key.as_ref())
            .is_some()
    }

    pub async fn get_api_key_and_headers(&self, model: &Model) -> ResolvedRequestAuth {
        let provider_config = self
            .provider_request_configs
            .lock()
            .unwrap()
            .get(&model.provider)
            .cloned();

        let api_key_from_storage = self.auth_storage.get_api_key(&model.provider, false).await;

        let api_key = api_key_from_storage.or_else(|| {
            provider_config.as_ref().and_then(|c| {
                c.api_key.as_ref().and_then(|k| {
                    resolve_config_value_or_throw(
                        k,
                        &format!("API key for provider \"{}\"", model.provider),
                    )
                    .ok()
                })
            })
        });

        let headers = {
            let provider_headers = provider_config
                .as_ref()
                .and_then(|c| c.headers.as_ref())
                .and_then(|h| {
                    let desc = format!("provider \"{}\"", model.provider);
                    resolve_headers_or_throw(Some(h), &desc).ok().flatten()
                });

            let model_key = format!("{}:{}", model.provider, model.id);
            let model_headers = self
                .model_request_headers
                .lock()
                .unwrap()
                .get(&model_key)
                .cloned()
                .and_then(|h| {
                    let desc = format!("model \"{}/{}\"", model.provider, model.id);
                    resolve_headers_or_throw(Some(&h), &desc).ok().flatten()
                });

            match (provider_headers, model_headers) {
                (Some(mut ph), Some(mh)) => {
                    ph.extend(mh);
                    Some(ph)
                }
                (Some(ph), None) => Some(ph),
                (None, Some(mh)) => Some(mh),
                (None, None) => None,
            }
        };

        ResolvedRequestAuth {
            ok: true,
            api_key,
            headers,
            error: None,
        }
    }

    pub fn get_provider_auth_status(&self, provider: &str) -> AuthStatus {
        let auth_status = self.auth_storage.get_auth_status(provider);
        if auth_status.source.is_some() {
            return auth_status;
        }

        let provider_api_key = self
            .provider_request_configs
            .lock()
            .unwrap()
            .get(provider)
            .and_then(|c| c.api_key.as_ref())
            .cloned();

        match provider_api_key {
            Some(key) if key.starts_with('!') => AuthStatus {
                configured: true,
                source: Some("models_json_command".to_string()),
                label: None,
            },
            Some(key) => {
                if std::env::var(&key).is_ok() {
                    AuthStatus {
                        configured: true,
                        source: Some("environment".to_string()),
                        label: Some(key),
                    }
                } else {
                    AuthStatus {
                        configured: true,
                        source: Some("models_json_key".to_string()),
                        label: None,
                    }
                }
            }
            None => auth_status,
        }
    }
}

impl RawProviderConfig {
    fn cost(&self) -> Option<&RawModelCost> {
        None
    }
}

struct CustomModelsResult {
    models: Vec<Model>,
    error: Option<String>,
}

/// Strip `//` line comments and trailing commas from JSON
fn strip_json_comments(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut in_string = false;
    let mut chars = input.char_indices().peekable();

    while let Some((_, c)) = chars.next() {
        match c {
            '"' => {
                result.push('"');
                in_string = !in_string;
            }
            '/' if !in_string => {
                if let Some(&(_, '/')) = chars.peek() {
                    chars.next();
                    // Skip until newline
                    while let Some(&(_, c)) = chars.peek() {
                        if c == '\n' {
                            break;
                        }
                        chars.next();
                    }
                } else {
                    result.push('/');
                }
            }
            ',' if !in_string => {
                // Skip trailing comma before ] or }
                if let Some(&(_, next)) = chars.peek() {
                    if next == '}' || next == ']' {
                        continue;
                    }
                }
                result.push(',');
            }
            _ => result.push(c),
        }
    }

    result
}
