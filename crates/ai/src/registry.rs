//! API Provider Registry

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::types::{Context, Model, StreamEvent, StreamOptions};

/// A provider function that streams responses
pub type StreamFn = Arc<
    dyn Send + Sync + for<'a> Fn(Model, Context, Option<StreamOptions>) -> tokio::sync::mpsc::Receiver<StreamEvent>,
>;

/// A registered API provider
#[derive(Clone)]
pub struct RegisteredProvider {
    pub api: String,
    pub stream: StreamFn,
    pub source_id: Option<String>,
}

/// Global provider registry
pub struct ApiProviderRegistry {
    providers: Arc<RwLock<HashMap<String, RegisteredProvider>>>,
}

impl Default for ApiProviderRegistry {
    fn default() -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl ApiProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, provider: RegisteredProvider) {
        if let Ok(mut providers) = self.providers.write() {
            providers.insert(provider.api.clone(), provider);
        }
    }

    pub fn get(&self, api: &str) -> Option<RegisteredProvider> {
        if let Ok(providers) = self.providers.read() {
            return providers.get(api).cloned();
        }
        None
    }

    pub fn unregister_source(&self, source_id: &str) {
        if let Ok(mut providers) = self.providers.write() {
            providers.retain(|_, p| p.source_id.as_deref() != Some(source_id));
        }
    }

    pub fn clear(&self) {
        if let Ok(mut providers) = self.providers.write() {
            providers.clear();
        }
    }

    pub fn list_apis(&self) -> Vec<String> {
        if let Ok(providers) = self.providers.read() {
            return providers.keys().cloned().collect();
        }
        vec![]
    }
}

use std::sync::LazyLock;

static GLOBAL_REGISTRY: LazyLock<ApiProviderRegistry> = LazyLock::new(|| {
    let reg = ApiProviderRegistry::new();
    // Auto-register built-in providers
    crate::providers::register::register_builtins_internal(&reg);
    reg
});

pub fn global_registry() -> &'static ApiProviderRegistry {
    &GLOBAL_REGISTRY
}
