//! Event bus - typed event emitter system

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type Handler = Arc<dyn Send + Sync + Fn(Arc<dyn Any + Send>) + 'static>;

/// Event bus for pub/sub communication
pub struct EventBus {
    handlers: Arc<Mutex<HashMap<String, Vec<Handler>>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Emit an event on a channel
    pub fn emit<T: Send + 'static>(&self, channel: &str, data: T) {
        let handlers = self.handlers.lock().unwrap();
        if let Some(channel_handlers) = handlers.get(channel) {
            let shared = Arc::new(data) as Arc<dyn Any + Send>;
            for handler in channel_handlers {
                (handler)(shared.clone());
            }
        }
    }

    /// Register a handler for a channel. Returns a cleanup function.
    pub fn on<T, F>(&self, channel: &str, handler: F) -> Box<dyn Fn() + Send>
    where
        T: Send + 'static,
        F: Send + Sync + 'static + Fn(&T),
    {
        let wrapped: Handler = Arc::new(move |data| {
            if let Some(val) = data.downcast_ref::<T>() {
                handler(val);
            }
        });

        {
            let mut handlers = self.handlers.lock().unwrap();
            handlers
                .entry(channel.to_string())
                .or_default()
                .push(wrapped.clone());
        }

        let channel = channel.to_string();
        let handlers = self.handlers.clone();
        Box::new(move || {
            if let Ok(mut h) = handlers.lock()
                && let Some(ch) = h.get_mut(&channel)
            {
                ch.retain(|h| !Arc::ptr_eq(h, &wrapped));
            }
        })
    }

    /// Remove all handlers
    pub fn clear(&self) {
        if let Ok(mut handlers) = self.handlers.lock() {
            handlers.clear();
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
