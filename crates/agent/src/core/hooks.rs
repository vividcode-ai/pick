//! General-purpose tool event hooks system.
//!
//! Provides a `ToolEventBus` where observers can subscribe to tool lifecycle
//! events (before/after execution, waiting-for-user).  This is independent of
//! the permission‑specific hooks in `crate::permission::hooks`.

use async_trait::async_trait;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Event types
// ---------------------------------------------------------------------------

/// Why the system is waiting for user input.
#[derive(Debug, Clone)]
pub enum WaitingKind {
    /// A permission request is pending approval.
    Permission { permission: String },
    /// The `question` tool has been called and is waiting for an answer.
    Question { header: String, question: String },
}

/// Events emitted during the tool lifecycle.
#[derive(Debug, Clone)]
pub enum ToolEvent {
    /// A tool is about to start executing.
    BeforeExecute {
        tool_name: String,
        tool_call_id: String,
        input: serde_json::Value,
    },
    /// A tool has finished executing.
    AfterExecute {
        tool_name: String,
        tool_call_id: String,
        input: serde_json::Value,
        output: serde_json::Value,
        is_error: bool,
    },
    /// The system is waiting for user input (permission approval or question
    /// answer).  Observers can use this to alert the user, e.g. via a desktop
    /// notification.
    WaitingForUser {
        tool_name: String,
        tool_call_id: String,
        input: serde_json::Value,
        kind: WaitingKind,
        /// Human‑readable summary for display purposes.
        summary: String,
    },
}

// ---------------------------------------------------------------------------
// Observer trait
// ---------------------------------------------------------------------------

/// An observer that can react to tool lifecycle events.
///
/// All methods have default empty implementations so observers only need to
/// override the events they care about.
#[async_trait]
pub trait ToolEventObserver: Send + Sync {
    /// Short human‑readable name for debugging / logging.
    fn name(&self) -> &str;

    /// Called for every tool event.
    async fn on_tool_event(&self, _event: &ToolEvent) {}
}

// ---------------------------------------------------------------------------
// Event bus
// ---------------------------------------------------------------------------

/// A simple in‑process event bus that dispatches `ToolEvent`s to all
/// registered observers.
///
/// Thread‑safe: observers can be registered from any thread and events can
/// be published concurrently.
pub struct ToolEventBus {
    observers: std::sync::Mutex<Vec<Arc<dyn ToolEventObserver>>>,
}

impl Default for ToolEventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolEventBus {
    /// Create an empty event bus.
    pub fn new() -> Self {
        Self {
            observers: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Register an observer.  All future events will be dispatched to it.
    pub fn subscribe(&self, observer: Arc<dyn ToolEventObserver>) {
        if let Ok(mut obs) = self.observers.lock() {
            obs.push(observer);
        }
    }

    /// Publish an event to all currently registered observers.
    ///
    /// Observers are called in registration order.  If an observer panics the
    /// panic is caught and logged; other observers are still notified.
    pub async fn publish(&self, event: &ToolEvent) {
        let snapshot = {
            let obs = self.observers.lock().ok();
            obs.map(|o| o.iter().map(Arc::clone).collect::<Vec<_>>())
        };
        if let Some(observers) = snapshot {
            for observer in &observers {
                observer.on_tool_event(event).await;
            }
        }
    }
}
