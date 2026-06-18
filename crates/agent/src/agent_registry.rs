//! Agent registry and lifecycle management

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::{RwLock, watch};
use pick_ai::types::Message;

use crate::core::agent_loop::{run_agent_loop, AgentLoopConfig, AgentRunResult};
use crate::inter_agent::{AgentStatus, InterAgentMessage, Mailbox};

/// Information about a running child agent
pub struct ChildAgent {
    pub id: String,
    pub name: String,
    pub handle: tokio::task::JoinHandle<Result<AgentRunResult, String>>,
    pub status_tx: watch::Sender<AgentStatus>,
    pub status_rx: watch::Receiver<AgentStatus>,
    pub mailbox: Arc<Mailbox>,
    pub cancel_tx: watch::Sender<bool>,
}

impl ChildAgent {
    fn new(
        id: String,
        name: String,
        handle: tokio::task::JoinHandle<Result<AgentRunResult, String>>,
        status_tx: watch::Sender<AgentStatus>,
        status_rx: watch::Receiver<AgentStatus>,
        mailbox: Arc<Mailbox>,
        cancel_tx: watch::Sender<bool>,
    ) -> Self {
        Self { id, name, handle, status_tx, status_rx, mailbox, cancel_tx }
    }
}

/// Agent registry managing child agent lifecycles.
pub struct AgentRegistry {
    agents: RwLock<HashMap<String, Arc<ChildAgent>>>,
    next_id: AtomicU64,
}

impl AgentRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            agents: RwLock::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        })
    }

    fn generate_id(&self) -> String {
        let n = self.next_id.fetch_add(1, Ordering::SeqCst);
        format!("child_{}", n)
    }

    /// Spawn a child agent in-process, returning its ID and handle.
    pub async fn spawn_child(
        self: &Arc<Self>,
        name: &str,
        config: AgentLoopConfig,
        initial_messages: Vec<Message>,
        parent_id: &str,
    ) -> Result<(String, Arc<ChildAgent>), String> {
        let id = self.generate_id();
        let (status_tx, status_rx) = watch::channel(AgentStatus::Running);
        let status_tx_for_task = status_tx.clone();
        let (cancel_tx, _cancel_rx) = watch::channel(false);
        let mailbox = Arc::new(Mailbox::new());
        let _registry = self.clone();
        let _child_name = name.to_string();
        let _child_id = id.clone();
        let parent_id = parent_id.to_string();
        let _config = config;

        // Spawn the agent loop in a tokio task
        let handle = tokio::spawn(async move {
            let result = run_agent_loop(_config, initial_messages).await;

            // Update status based on result
            match &result {
                Ok(_) => {
                    let _ = status_tx_for_task.send(AgentStatus::Completed);
                }
                Err(e) => {
                    let _ = status_tx_for_task.send(AgentStatus::Errored(e.clone()));
                }
            }

            result
        });

        let child = Arc::new(ChildAgent::new(
            id.clone(),
            name.to_string(),
            handle,
            status_tx,
            status_rx,
            mailbox.clone(),
            cancel_tx,
        ));

        // Register in the registry
        {
            let mut agents = self.agents.write().await;
            agents.insert(id.clone(), child.clone());
        }

        // Start completion watcher
        let watcher_registry = self.clone();
        let watcher_child = child.clone();
        let watcher_parent_id = parent_id.clone();
        tokio::spawn(async move {
            let mut rx = watcher_child.status_rx.clone();
            loop {
                if rx.changed().await.is_err() {
                    break;
                }
                let status = rx.borrow().clone();
                if status == AgentStatus::Completed || matches!(status, AgentStatus::Errored(_)) {
                    // Notify parent via mailbox
                    if let Some(parent_agent) = watcher_registry.get(&watcher_parent_id).await {
                        let msg = InterAgentMessage {
                            from: watcher_child.id.clone(),
                            to: watcher_parent_id.clone(),
                            content: format!(
                                "Agent `{}` ({}) has completed with status: {:?}",
                                watcher_child.name, watcher_child.id, status
                            ),
                            trigger_turn: false,
                        };
                        parent_agent.mailbox.enqueue(msg).await;
                    }
                    break;
                }
            }
        });

        Ok((id, child))
    }

    /// Get a child agent by ID
    pub async fn get(&self, id: &str) -> Option<Arc<ChildAgent>> {
        let agents = self.agents.read().await;
        agents.get(id).cloned()
    }

    /// Remove and cleanup a child agent
    pub async fn remove(&self, id: &str) {
        let mut agents = self.agents.write().await;
        if let Some(child) = agents.remove(id) {
            let _ = child.cancel_tx.send(true);
        }
    }

    /// List all active child agent IDs
    pub async fn list(&self) -> Vec<String> {
        let agents = self.agents.read().await;
        agents.keys().cloned().collect()
    }

    /// Check if an agent ID exists
    pub async fn exists(&self, id: &str) -> bool {
        let agents = self.agents.read().await;
        agents.contains_key(id)
    }

    /// Send an inter-agent message
    pub async fn send_message(&self, msg: InterAgentMessage) -> Result<(), String> {
        let agents = self.agents.read().await;
        if let Some(target) = agents.get(&msg.to) {
            target.mailbox.enqueue(msg).await;
            Ok(())
        } else {
            Err(format!("Agent '{}' not found", msg.to))
        }
    }
}
