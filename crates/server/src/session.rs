use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pick_agent::core::state::AgentTool;
use pick_agent::session::entries::{SessionEntry, SessionEntryKind, SessionInfoEntry};
use pick_agent::session::manager::SessionManager as AgentSessionManager;
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
        title: String,
        model_id: String,
        provider: String,
        system_prompt: String,
        tools: Vec<AgentTool>,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id,
            title,
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
    cwd: PathBuf,
}

impl SessionManager {
    pub fn new(session_dir: PathBuf) -> Self {
        let cwd = session_dir
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        Self {
            sessions: RwLock::new(HashMap::new()),
            session_dir,
            cwd,
        }
    }

    pub fn new_with_cwd(session_dir: PathBuf, cwd: PathBuf) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            session_dir,
            cwd,
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
    ) -> (String, String) {
        match AgentSessionManager::create(
            self.cwd.clone(),
            Some(self.session_dir.clone()),
            Some(model_id.clone()),
            Some(provider.clone()),
        )
        .await
        {
            Ok(agent) => {
                let id = agent
                    .header()
                    .map(|h| h.id.clone())
                    .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
                let title = agent.get_session_name().unwrap_or("Session").to_string();
                let path = agent
                    .session_path()
                    .map(|p| p.to_string_lossy().to_string());
                let session = AgentSession {
                    id: id.clone(),
                    title: title.clone(),
                    model_id,
                    provider,
                    system_prompt,
                    tools,
                    messages: Vec::new(),
                    created_at: agent.header().map(|h| h.created_at).unwrap_or_default(),
                    updated_at: agent.header().map(|h| h.updated_at).unwrap_or_default(),
                    status: "idle".to_string(),
                    fork_parent_id: None,
                    session_path: path,
                    persisted_messages_count: 0,
                };
                self.sessions.write().await.insert(id.clone(), session);
                (id, title)
            }
            Err(e) => {
                error!("Failed to create session: {}", e);
                let id = uuid::Uuid::now_v7().to_string();
                let title = format!("Session - {}", chrono::Utc::now().format("%Y-%m-%d %H:%M"));
                let session = AgentSession::new(
                    id.clone(),
                    title.clone(),
                    model_id,
                    provider,
                    system_prompt,
                    tools,
                );
                self.sessions.write().await.insert(id.clone(), session);
                (id, title)
            }
        }
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

            // Persist new messages via agent if we have a path
            if let Some(path) = s.session_path.clone() {
                let new_msgs: Vec<Message> = s.messages[prev_count..].to_vec();
                if !new_msgs.is_empty() {
                    drop(sessions);
                    self.append_messages_to_jsonl(&path, &new_msgs).await;
                    let mut sessions = self.sessions.write().await;
                    if let Some(s) = sessions.get_mut(id) {
                        s.persisted_messages_count = s.messages.len();
                    }
                    return;
                }
            }
            // No path or no new messages, just update count
            s.persisted_messages_count = s.messages.len();
        }
    }

    async fn append_messages_to_jsonl(&self, path: &str, messages: &[Message]) {
        let path = PathBuf::from(path);

        let mut content = tokio::fs::read_to_string(&path).await.unwrap_or_default();

        // Find current leaf from existing entries to maintain parent chain
        let leaf_id = {
            let entries: Vec<SessionEntry> = content
                .lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect();
            let child_ids: HashSet<&str> = entries
                .iter()
                .filter_map(|e| e.parent_id.as_deref())
                .collect();
            entries
                .iter()
                .filter(|e| !child_ids.contains(e.id.as_str()))
                .max_by_key(|e| e.timestamp)
                .map(|e| e.id.clone())
        };

        let mut prev_id = leaf_id;
        for msg in messages {
            let mut entry: SessionEntry = msg.into();
            entry.parent_id = prev_id;
            prev_id = Some(entry.id.clone());
            if let Ok(line) = serde_json::to_string(&entry) {
                content.push_str(&line);
                content.push('\n');
            }
        }
        let _ = tokio::fs::write(&path, &content).await;
    }

    pub async fn delete(&self, id: &str) -> bool {
        // Remove file from disk
        {
            let sessions = self.sessions.read().await;
            if let Some(s) = sessions.get(id)
                && let Some(path) = &s.session_path
            {
                let p = PathBuf::from(path);
                let _ = tokio::fs::remove_file(&p).await;
            }
        }
        // Remove from memory
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
        // For title persistence, we need the path before locking
        let needs_persist = if title.is_some() {
            let sessions = self.sessions.read().await;
            sessions.get(id).and_then(|s| s.session_path.clone())
        } else {
            None
        };

        let mut sessions = self.sessions.write().await;
        if let Some(s) = sessions.get_mut(id) {
            if let Some(t) = title {
                s.title = t.clone();
                s.updated_at = chrono::Utc::now().timestamp_millis();
                if let Some(path) = needs_persist {
                    drop(sessions);
                    self.append_session_info_to_jsonl(&path, &t).await;
                    return true;
                }
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

    async fn append_session_info_to_jsonl(&self, path: &str, name: &str) {
        let path = PathBuf::from(path);
        let mut content = tokio::fs::read_to_string(&path).await.unwrap_or_default();

        // Find current leaf to maintain parent chain
        let leaf_id = {
            let entries: Vec<SessionEntry> = content
                .lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect();
            let child_ids: HashSet<&str> = entries
                .iter()
                .filter_map(|e| e.parent_id.as_deref())
                .collect();
            entries
                .iter()
                .filter(|e| !child_ids.contains(e.id.as_str()))
                .max_by_key(|e| e.timestamp)
                .map(|e| e.id.clone())
        };

        let entry = SessionEntry {
            id: uuid::Uuid::now_v7().to_string(),
            parent_id: leaf_id,
            timestamp: chrono::Utc::now().timestamp_millis(),
            kind: SessionEntryKind::SessionInfo(SessionInfoEntry {
                name: name.to_string(),
            }),
        };
        if let Ok(line) = serde_json::to_string(&entry) {
            content.push_str(&line);
            content.push('\n');
        }
        let _ = tokio::fs::write(path, &content).await;
    }

    pub async fn fork(&self, id: &str) -> Option<(String, String)> {
        let (
            source_path,
            title,
            model_id,
            provider,
            system_prompt,
            tools,
            source_messages,
            source_id,
        ) = {
            let sessions = self.sessions.read().await;
            let source = sessions.get(id)?;
            (
                source.session_path.clone()?,
                format!("{} (fork)", source.title),
                source.model_id.clone(),
                source.provider.clone(),
                source.system_prompt.clone(),
                source.tools.clone(),
                source.messages.clone(),
                source.id.clone(),
            )
        };

        let msg_count = source_messages.len();
        match AgentSessionManager::fork_from(
            PathBuf::from(&source_path),
            self.cwd.clone(),
            Some(model_id.clone()),
            Some(provider.clone()),
        )
        .await
        {
            Ok(agent) => {
                let new_id = agent
                    .header()
                    .map(|h| h.id.clone())
                    .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
                let path = agent
                    .session_path()
                    .map(|p| p.to_string_lossy().to_string());
                let now = chrono::Utc::now().timestamp_millis();
                let session = AgentSession {
                    id: new_id.clone(),
                    title: title.clone(),
                    model_id,
                    provider,
                    system_prompt,
                    tools,
                    messages: source_messages.clone(),
                    created_at: now,
                    updated_at: now,
                    status: "idle".to_string(),
                    fork_parent_id: Some(source_id),
                    session_path: path,
                    persisted_messages_count: msg_count,
                };
                self.sessions.write().await.insert(new_id.clone(), session);
                Some((new_id, title))
            }
            Err(e) => {
                error!("Failed to fork session {}: {}", id, e);
                None
            }
        }
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

    /// Compact the session JSONL file on disk by removing non-essential entries
    /// (compaction summaries, labels, etc.) while preserving message history.
    pub async fn compact_session(&self, id: &str) -> bool {
        let path = {
            let sessions = self.sessions.read().await;
            sessions.get(id).and_then(|s| s.session_path.clone())
        };
        if let Some(path) = path {
            let agent_mgr = AgentSessionManager::open(PathBuf::from(path), PathBuf::new()).await;
            if let Ok(mgr) = agent_mgr {
                mgr.compact().await.is_ok()
            } else {
                false
            }
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
