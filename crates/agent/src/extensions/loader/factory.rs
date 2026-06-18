//! Extension Factory Registry

use std::sync::{Arc, LazyLock, Mutex};

use super::super::types::ExtensionFactory;

/// Global registry for extension factories
pub struct ExtensionFactoryRegistry {
    factories: Mutex<Vec<Arc<dyn ExtensionFactory>>>,
}

impl ExtensionFactoryRegistry {
    fn new() -> Self {
        Self {
            factories: Mutex::new(Vec::new()),
        }
    }

    pub fn register(&self, factory: Arc<dyn ExtensionFactory>) {
        if let Ok(mut factories) = self.factories.lock() {
            factories.push(factory);
        }
    }

    pub fn get_all(&self) -> Vec<Arc<dyn ExtensionFactory>> {
        self.factories
            .lock()
            .map(|f| f.clone())
            .unwrap_or_default()
    }
}

static GLOBAL_EXTENSION_FACTORIES: LazyLock<ExtensionFactoryRegistry> =
    LazyLock::new(ExtensionFactoryRegistry::new);

/// Get the global extension factory registry
pub fn global_extension_registry() -> &'static ExtensionFactoryRegistry {
    &GLOBAL_EXTENSION_FACTORIES
}

/// Register an extension factory globally (called at startup)
pub fn register_extension_factory(factory: Arc<dyn ExtensionFactory>) {
    global_extension_registry().register(factory);
}
