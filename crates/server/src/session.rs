use std::collections::HashMap;
use std::sync::Arc;

use pick_agent::core::state::AgentTool;
use pick_ai::types::Message;
use tokio::sync::{RwLock, oneshot};

#[derive(Clone)]
pub struct PendingApproval {
    pub response_tx: Arc<tokio::sync::Mutex<Option<oneshot::Sender<bool>>>>,
}

#[derive(Clone)]
pub struct PendingQuestion {
    #[allow(clippy::type_complexity)]
    pub response_tx:
        Arc<tokio::sync::Mutex<Option<oneshot::Sender<Result<Vec<Vec<String>>, String>>>>>,
}

#[derive(Clone)]
pub struct AgentSession {
    pub id: String,
    pub model_id: String,
    pub provider: String,
    pub system_prompt: String,
    pub tools: Vec<AgentTool>,
    pub messages: Vec<Message>,
}

impl AgentSession {
    pub fn new(
        id: String,
        model_id: String,
        provider: String,
        system_prompt: String,
        tools: Vec<AgentTool>,
    ) -> Self {
        Self {
            id,
            model_id,
            provider,
            system_prompt,
            tools,
            messages: Vec::new(),
        }
    }
}

pub struct SessionManager {
    sessions: RwLock<HashMap<String, AgentSession>>,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    pub async fn create(
        &self,
        model_id: String,
        provider: String,
        system_prompt: String,
        tools: Vec<AgentTool>,
    ) -> String {
        let id = uuid::Uuid::now_v7().to_string();
        let session = AgentSession::new(id.clone(), model_id, provider, system_prompt, tools);
        self.sessions.write().await.insert(id.clone(), session);
        id
    }

    pub async fn get(&self, id: &str) -> Option<AgentSession> {
        let sessions = self.sessions.read().await;
        sessions.get(id).cloned()
    }

    pub async fn update_messages(&self, id: &str, messages: Vec<Message>) {
        let mut sessions = self.sessions.write().await;
        if let Some(s) = sessions.get_mut(id) {
            s.messages = messages;
        }
    }

    pub async fn delete(&self, id: &str) -> bool {
        self.sessions.write().await.remove(id).is_some()
    }

    pub async fn list(&self) -> Vec<String> {
        self.sessions.read().await.keys().cloned().collect()
    }
}
