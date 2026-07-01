use std::cmp::Reverse;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use pick_agent::core::state::AgentTool;
use pick_ai::types::Message;
use serde::Serialize;
use tokio::sync::{RwLock, oneshot, watch};
use utoipa::ToSchema;

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

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub model_id: String,
    pub provider: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub message_count: usize,
    pub status: String,
    pub fork_parent_id: Option<String>,
}

#[derive(Clone)]
pub struct AgentSession {
    pub id: String,
    pub title: String,
    pub model_id: String,
    pub provider: String,
    pub system_prompt: String,
    pub tools: Vec<AgentTool>,
    pub messages: Vec<Message>,
    pub created_at: i64,
    pub updated_at: i64,
    pub status: String,
    pub fork_parent_id: Option<String>,
    pub session_path: Option<String>,
}

impl AgentSession {
    pub fn new(
        id: String,
        model_id: String,
        provider: String,
        system_prompt: String,
        tools: Vec<AgentTool>,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id,
            title: format!("Session - {}", chrono::Utc::now().format("%Y-%m-%d %H:%M")),
            model_id,
            provider,
            system_prompt,
            tools,
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
            status: "idle".to_string(),
            fork_parent_id: None,
            session_path: None,
        }
    }

    pub fn to_info(&self) -> SessionInfo {
        SessionInfo {
            id: self.id.clone(),
            title: self.title.clone(),
            model_id: self.model_id.clone(),
            provider: self.provider.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            message_count: self.messages.len(),
            status: self.status.clone(),
            fork_parent_id: self.fork_parent_id.clone(),
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

    pub async fn insert_session(&self, session: AgentSession) {
        self.sessions
            .write()
            .await
            .insert(session.id.clone(), session);
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
            s.updated_at = chrono::Utc::now().timestamp_millis();
        }
    }

    pub async fn delete(&self, id: &str) -> bool {
        self.sessions.write().await.remove(id).is_some()
    }

    pub async fn list(&self) -> Vec<String> {
        self.sessions.read().await.keys().cloned().collect()
    }

    pub async fn list_info(&self) -> Vec<SessionInfo> {
        let sessions = self.sessions.read().await;
        let mut infos: Vec<SessionInfo> = sessions.values().map(|s| s.to_info()).collect();
        infos.sort_by_key(|b| Reverse(b.updated_at));
        infos
    }

    pub async fn update_session(
        &self,
        id: &str,
        title: Option<String>,
        model_id: Option<String>,
        provider: Option<String>,
    ) -> bool {
        let mut sessions = self.sessions.write().await;
        if let Some(s) = sessions.get_mut(id) {
            if let Some(t) = title {
                s.title = t;
            }
            if let Some(m) = model_id {
                s.model_id = m;
            }
            if let Some(p) = provider {
                s.provider = p;
            }
            s.updated_at = chrono::Utc::now().timestamp_millis();
            true
        } else {
            false
        }
    }

    pub async fn fork(&self, id: &str) -> Option<String> {
        let sessions = self.sessions.read().await;
        let source = sessions.get(id)?;
        let new_id = uuid::Uuid::now_v7().to_string();
        let now = chrono::Utc::now().timestamp_millis();
        let forked = AgentSession {
            id: new_id.clone(),
            title: format!("{} (fork)", source.title),
            model_id: source.model_id.clone(),
            provider: source.provider.clone(),
            system_prompt: source.system_prompt.clone(),
            tools: source.tools.clone(),
            messages: source.messages.clone(),
            created_at: now,
            updated_at: now,
            status: "idle".to_string(),
            fork_parent_id: Some(source.id.clone()),
            session_path: None,
        };
        drop(sessions);
        self.sessions.write().await.insert(new_id.clone(), forked);
        Some(new_id)
    }

    pub async fn set_status(&self, id: &str, status: &str) -> bool {
        let mut sessions = self.sessions.write().await;
        if let Some(s) = sessions.get_mut(id) {
            s.status = status.to_string();
            s.updated_at = chrono::Utc::now().timestamp_millis();
            true
        } else {
            false
        }
    }

    pub async fn get_messages(
        &self,
        id: &str,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Option<(Vec<Message>, usize)> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(id)?;
        let total = session.messages.len();
        let offset = offset.unwrap_or(0);
        let limit = limit.unwrap_or(usize::MAX);
        let msgs: Vec<Message> = session
            .messages
            .iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect();
        Some((msgs, total))
    }
}

type PendingQuestionSenders =
    Arc<Mutex<HashMap<String, oneshot::Sender<Result<Vec<Vec<String>>, String>>>>>;

#[derive(Clone)]
pub struct SseSessionState {
    pub event_tx: tokio::sync::mpsc::UnboundedSender<
        Result<axum::response::sse::Event, std::convert::Infallible>,
    >,
    pub cancel_tx: Option<watch::Sender<bool>>,
    pub pending_approvals: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
    pub pending_questions: PendingQuestionSenders,
}
