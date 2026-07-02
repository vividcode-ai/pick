use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pick_agent::core::state::AgentTool;
use pick_agent::session::entries::{
    SessionEntry, SessionEntryKind, SessionHeader, SessionInfoEntry,
};
use pick_ai::types::Message;
use serde::Serialize;
use tokio::sync::{RwLock, oneshot, watch};
use tracing::error;
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
    pub persisted_messages_count: usize,
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
            persisted_messages_count: 0,
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
    session_dir: PathBuf,
}

impl SessionManager {
    pub fn new(session_dir: PathBuf) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            session_dir,
        }
    }

    async fn write_header_and_info(&self, session: &AgentSession) -> Result<String, String> {
        let path = self.session_dir.join(format!("{}.jsonl", session.id));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create dir: {}", e))?;
        }
        let header = SessionHeader {
            id: session.id.clone(),
            version: 1,
            created_at: session.created_at,
            updated_at: session.updated_at,
            cwd: None,
            model: Some(session.model_id.clone()),
            provider: Some(session.provider.clone()),
        };
        let header_line =
            serde_json::to_string(&header).map_err(|e| format!("Serialize header: {}", e))?;
        let info_entry = SessionEntry {
            id: uuid::Uuid::now_v7().to_string(),
            parent_id: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
            kind: SessionEntryKind::SessionInfo(SessionInfoEntry {
                name: session.title.clone(),
            }),
        };
        let info_line =
            serde_json::to_string(&info_entry).map_err(|e| format!("Serialize info: {}", e))?;
        let content = format!("{}\n{}\n", header_line, info_line);
        tokio::fs::write(&path, content)
            .await
            .map_err(|e| format!("Write session file: {}", e))?;
        Ok(path.to_string_lossy().to_string())
    }

    async fn append_messages_to_disk(
        &self,
        session: &AgentSession,
        start_index: usize,
    ) -> Result<(), String> {
        let path = match &session.session_path {
            Some(p) => std::path::PathBuf::from(p),
            None => return Err("No session path".to_string()),
        };
        let new_messages: Vec<&Message> = session.messages.iter().skip(start_index).collect();
        if new_messages.is_empty() {
            return Ok(());
        }
        let mut content = String::new();
        for msg in new_messages {
            let entry: SessionEntry = msg.into();
            let line =
                serde_json::to_string(&entry).map_err(|e| format!("Serialize message: {}", e))?;
            content.push_str(&line);
            content.push('\n');
        }
        let mut file_content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
        file_content.push_str(&content);
        tokio::fs::write(&path, &file_content)
            .await
            .map_err(|e| format!("Write messages: {}", e))?;
        Ok(())
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
    ) -> (String, String) {
        let id = uuid::Uuid::now_v7().to_string();
        let mut session = AgentSession::new(id.clone(), model_id, provider, system_prompt, tools);
        let title = session.title.clone();
        match self.write_header_and_info(&session).await {
            Ok(path) => {
                session.session_path = Some(path);
            }
            Err(e) => {
                error!("Failed to persist session {}: {}", id, e);
            }
        }
        self.sessions.write().await.insert(id.clone(), session);
        (id, title)
    }

    pub async fn get(&self, id: &str) -> Option<AgentSession> {
        let sessions = self.sessions.read().await;
        sessions.get(id).cloned()
    }

    pub async fn update_messages(&self, id: &str, messages: Vec<Message>) {
        let mut sessions = self.sessions.write().await;
        if let Some(s) = sessions.get_mut(id) {
            let prev_count = s.persisted_messages_count;
            s.messages = messages;
            s.updated_at = chrono::Utc::now().timestamp_millis();
            if s.session_path.is_some() {
                let session_copy = s.clone();
                drop(sessions);
                if let Err(e) = self
                    .append_messages_to_disk(&session_copy, prev_count)
                    .await
                {
                    error!("Failed to persist messages for session {}: {}", id, e);
                }
                let mut sessions = self.sessions.write().await;
                if let Some(s) = sessions.get_mut(id) {
                    s.persisted_messages_count = s.messages.len();
                }
            }
        }
    }

    pub async fn delete(&self, id: &str) -> bool {
        let path = {
            let sessions = self.sessions.read().await;
            sessions.get(id).and_then(|s| s.session_path.clone())
        };
        // Remove from memory
        let removed = self.sessions.write().await.remove(id).is_some();
        // Remove file from disk
        if let Some(p) = path {
            let p = std::path::PathBuf::from(&p);
            if p.exists() {
                let _ = tokio::fs::remove_file(&p).await;
            }
        }
        removed
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

    pub async fn fork(&self, id: &str) -> Option<(String, String)> {
        let sessions = self.sessions.read().await;
        let source = sessions.get(id)?;
        let new_id = uuid::Uuid::now_v7().to_string();
        let now = chrono::Utc::now().timestamp_millis();
        let title = format!("{} (fork)", source.title);
        let mut forked = AgentSession {
            id: new_id.clone(),
            title: title.clone(),
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
            persisted_messages_count: 0,
        };
        drop(sessions);
        match self.write_header_and_info(&forked).await {
            Ok(path) => {
                forked.session_path = Some(path);
            }
            Err(e) => {
                error!("Failed to persist forked session {}: {}", new_id, e);
            }
        }
        self.sessions.write().await.insert(new_id.clone(), forked);
        Some((new_id, title))
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
