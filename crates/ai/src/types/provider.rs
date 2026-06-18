//! Provider trait definitions for AI services.

use super::model::Api;

/// A provider that can stream AI responses (simplified trait)
pub trait ApiProvider: Send + Sync {
    fn api(&self) -> Api;
}

/// Registry for API providers
#[derive(Default, Clone)]
pub struct ApiProviderRegistry {
    providers: std::sync::Arc<std::sync::RwLock<Vec<Box<dyn ApiProvider>>>>,
}

impl ApiProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, provider: Box<dyn ApiProvider>) {
        if let Ok(mut providers) = self.providers.write() {
            providers.push(provider);
        }
    }

    pub fn len(&self) -> usize {
        if let Ok(providers) = self.providers.read() {
            return providers.len();
        }
        0
    }
}
