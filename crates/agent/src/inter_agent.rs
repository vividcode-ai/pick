//! Inter-agent communication types

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

/// A message sent between agents
#[derive(Debug, Clone)]
pub struct InterAgentMessage {
    pub from: String,
    pub to: String,
    pub content: String,
    pub trigger_turn: bool,
}

/// Status of an agent
#[derive(Debug, Clone, PartialEq)]
pub enum AgentStatus {
    Running,
    Completed,
    Errored(String),
}

/// Per-agent mailbox for receiving inter-agent messages.
pub struct Mailbox {
    notify_tx: tokio::sync::watch::Sender<()>,
    notify_rx: tokio::sync::watch::Receiver<()>,
    pending: Arc<Mutex<VecDeque<InterAgentMessage>>>,
}

impl Mailbox {
    pub fn new() -> Self {
        let (notify_tx, notify_rx) = tokio::sync::watch::channel(());
        Self {
            notify_tx,
            notify_rx,
            pending: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Enqueue a message and notify subscribers
    pub async fn enqueue(&self, msg: InterAgentMessage) {
        let mut pending = self.pending.lock().await;
        pending.push_back(msg);
        let _ = self.notify_tx.send(());
    }

    /// Drain all pending messages
    pub async fn drain(&self) -> Vec<InterAgentMessage> {
        let mut pending = self.pending.lock().await;
        pending.drain(..).collect()
    }

    /// Check if there are pending messages
    pub async fn has_pending(&self) -> bool {
        let pending = self.pending.lock().await;
        !pending.is_empty()
    }

    /// Subscribe to new mail notifications
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<()> {
        self.notify_rx.clone()
    }
}

impl Default for Mailbox {
    fn default() -> Self {
        Self::new()
    }
}
