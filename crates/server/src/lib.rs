pub mod docs;
pub mod events;
pub mod files;
pub mod mcp_routes;
pub mod plugins;
pub mod pty;
pub mod rest;
pub mod routes;
pub mod session;
pub mod spa;
pub mod sse;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::routing::{delete, get, post};
use pick_agent::session::GoalManager;
use pick_agent::session::entries::SessionEntryKind;
use pick_ai::types::Message;
use pick_mcp::McpManager;
use pick_mcp::config::McpServerConfig;
use pty::PtyManager;
use session::SseSessionState;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tracing::{debug, info};

pub struct AppState {
    pub session_manager: session::SessionManager,
    pub sse_sessions: RwLock<HashMap<String, SseSessionState>>,
    pub config: ServerConfig,
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub api_keys: HashMap<String, String>,
    pub pty_manager: PtyManager,
    pub mcp_manager: McpManager,
    pub mcp_configs: RwLock<Vec<McpServerConfig>>,
}

impl AppState {
    pub fn new(config: ServerConfig) -> Self {
        let cwd = config
            .cwd
            .as_deref()
            .map(Path::new)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let session_dir = cwd.join(".pick").join("sessions");
        Self {
            session_manager: session::SessionManager::new(session_dir),
            sse_sessions: RwLock::new(HashMap::new()),
            config,
            default_provider: None,
            default_model: None,
            api_keys: HashMap::new(),
            pty_manager: PtyManager::new(),
            mcp_manager: McpManager::new(),
            mcp_configs: RwLock::new(Vec::new()),
        }
    }

    pub fn build_system_prompt(&self, provider: &str, model_id: &str) -> String {
        format!(
            "You are Pick, an AI-powered coding assistant.\n\
             You help users with software engineering tasks.\n\
             You can use tools to read, write, edit files, run commands, search code, etc.\n\
             Provider: {}  Model: {}",
            provider, model_id
        )
    }

    pub fn get_tools(&self) -> Vec<pick_agent::core::state::AgentTool> {
        use pick_agent::tools::registry;
        let gm = Arc::new(GoalManager::new());
        let agent_mode: Option<String> = None;
        registry::create_coding_tools_with_goal_manager(agent_mode, gm)
    }

    fn home_dir() -> Option<std::path::PathBuf> {
        #[cfg(target_os = "windows")]
        {
            std::env::var("USERPROFILE")
                .ok()
                .map(std::path::PathBuf::from)
        }
        #[cfg(not(target_os = "windows"))]
        {
            std::env::var("HOME").ok().map(std::path::PathBuf::from)
        }
    }

    pub async fn load_persisted_sessions(&self, cwd: &Path) {
        let project_dir = cwd.join(".pick").join("sessions");
        if project_dir.exists() {
            info!("Loading session metadata from {}", project_dir.display());
            self.load_from_dir(&project_dir).await;
        }
        if let Some(home) = Self::home_dir() {
            let global_dir = home.join(".pick").join("agent").join("sessions");
            if global_dir.exists() {
                info!("Loading session metadata from {}", global_dir.display());
                self.load_from_dir(&global_dir).await;
            }
        }
    }

    /// Ensure a session's messages are loaded from disk (lazy load).
    /// No-op if messages already loaded or session is new (no path).
    pub async fn ensure_session_messages(&self, id: &str) {
        let has_path = {
            let session = self.session_manager.get(id).await;
            match session {
                Some(s) => !s.messages.is_empty() || s.session_path.is_none(),
                None => true,
            }
        };
        if has_path {
            return;
        }
        let session = self.session_manager.get(id).await;
        let path = session.and_then(|s| s.session_path.clone());
        let path = match path {
            Some(p) => p,
            None => return,
        };
        let path = std::path::PathBuf::from(&path);
        if let Some(loaded) = self.load_messages_from_file(&path).await {
            self.session_manager
                .update_messages(id, loaded.messages)
                .await;
        }
    }

    /// Scan dir and load only session metadata (header + session_info entry).
    async fn load_from_dir(&self, dir: &Path) {
        use tokio::fs;
        let mut rd = match fs::read_dir(dir).await {
            Ok(r) => r,
            Err(_) => return,
        };
        let mut files = Vec::new();
        loop {
            match rd.next_entry().await {
                Ok(Some(entry)) => {
                    if entry.path().extension() == Some(std::ffi::OsStr::new("jsonl")) {
                        files.push(entry.path());
                    }
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }
        for path in files {
            if let Some(session) = self.load_meta_only(&path).await {
                self.session_manager.insert_session(session).await;
                debug!("Loaded session meta: {}", path.display());
            }
        }
    }

    /// Read only the header and title from a JSONL file (line 1 + first session_info entry).
    async fn load_meta_only(&self, path: &Path) -> Option<session::AgentSession> {
        use pick_agent::session::entries::{SessionEntry, SessionHeader};

        let content = tokio::fs::read_to_string(path).await.ok()?;
        let mut lines = content.lines();

        let header_line = lines.next()?;
        let header: SessionHeader = serde_json::from_str(header_line).ok()?;

        let mut title = format!("Session - {}", chrono::Utc::now().format("%Y-%m-%d %H:%M"));

        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            let entry: SessionEntry = serde_json::from_str(line).ok()?;
            if let SessionEntryKind::SessionInfo(info) = &entry.kind {
                title = info.name.clone();
                break;
            }
        }

        let model_id = header
            .model
            .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
        let provider = header.provider.unwrap_or_else(|| "anthropic".to_string());

        Some(session::AgentSession {
            id: header.id,
            title,
            model_id,
            provider,
            system_prompt: String::new(),
            tools: Vec::new(),
            messages: Vec::new(),
            created_at: header.created_at,
            updated_at: header.updated_at,
            status: "idle".to_string(),
            fork_parent_id: None,
            session_path: Some(path.to_string_lossy().to_string()),
            persisted_messages_count: 0,
        })
    }

    /// Load all messages from a JSONL file (for lazy loading).
    async fn load_messages_from_file(&self, path: &Path) -> Option<session::AgentSession> {
        use pick_agent::session::entries::{SessionEntry, SessionHeader};

        let content = tokio::fs::read_to_string(path).await.ok()?;
        let mut lines = content.lines();

        let header_line = lines.next()?;
        let header: SessionHeader = serde_json::from_str(header_line).ok()?;

        let mut messages = Vec::new();

        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            let entry: SessionEntry = serde_json::from_str(line).ok()?;
            if let SessionEntryKind::Message(_) = &entry.kind {
                if let Ok(msg) = Message::try_from(&entry) {
                    messages.push(msg);
                }
            }
        }

        let msg_count = messages.len();
        let model_id = header
            .model
            .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
        let provider = header.provider.unwrap_or_else(|| "anthropic".to_string());

        Some(session::AgentSession {
            id: header.id,
            title: String::new(),
            model_id,
            provider,
            system_prompt: String::new(),
            tools: Vec::new(),
            messages,
            created_at: header.created_at,
            updated_at: header.updated_at,
            status: "idle".to_string(),
            fork_parent_id: None,
            session_path: Some(path.to_string_lossy().to_string()),
            persisted_messages_count: msg_count,
        })
    }
}

#[derive(Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub pty_ws_port: u16,
    pub cwd: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            pty_ws_port: 9000,
            cwd: None,
        }
    }
}

pub fn create_app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(rest::health))
        .route(
            "/sessions",
            get(rest::list_sessions).post(rest::create_session),
        )
        .route(
            "/sessions/{id}",
            get(rest::get_session)
                .delete(rest::delete_session)
                .patch(rest::update_session),
        )
        .route("/sessions/{id}/fork", post(rest::fork_session))
        .route("/sessions/{id}/messages", get(rest::get_session_messages))
        .route("/sessions/{id}/summarize", post(rest::summarize_session))
        .route("/sessions/{id}/status", get(rest::get_session_status))
        .route("/providers", get(rest::list_providers))
        .route("/events/{session_id}", get(sse::handle_sse))
        .route("/ask", post(routes::ask))
        .route("/cancel", post(routes::cancel))
        .route("/approve", post(routes::approve))
        .route("/answer_question", post(routes::answer_question))
        .route("/files/content", get(files::read_file_handler))
        .route("/files/list", get(files::list_dir_handler))
        .route("/find/text", get(files::find_text_handler))
        .route("/find/files", get(files::find_files_handler))
        .route(
            "/pty",
            post(pty::create_pty_handler).get(pty::list_pty_handler),
        )
        .route("/pty/{id}", delete(pty::destroy_pty_handler))
        .route("/plugins", get(plugins::list_plugins))
        .route(
            "/mcp",
            get(mcp_routes::list_mcp_servers).post(mcp_routes::add_mcp_server),
        )
        .route("/mcp/{name}", delete(mcp_routes::remove_mcp_server))
        .route(
            "/mcp/{name}/reconnect",
            post(mcp_routes::reconnect_mcp_server),
        )
        .route("/openapi.json", get(docs::openapi_json))
        .route("/docs", get(docs::docs_ui))
        .fallback(spa::spa_handler)
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn run_server(config: ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(AppState::new(config.clone()));

    // Load persisted sessions from disk
    let cwd = config
        .cwd
        .as_deref()
        .map(Path::new)
        .unwrap_or_else(|| Path::new("."));
    state.load_persisted_sessions(cwd).await;

    let app = create_app(state.clone());

    // Start PTY WebSocket server in background
    let pty_port = config.pty_ws_port;
    let pty_manager = state.pty_manager.clone();
    tokio::spawn(async move {
        let _ = pty::start_pty_ws_server(Arc::new(pty_manager), pty_port).await;
    });

    let addr = format!("{}:{}", config.host, config.port);
    info!(
        "pick-server starting on {} (PTY WS on port {})",
        addr, pty_port
    );

    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

pub async fn run_server_on_listener(
    listener: TcpListener,
    config: ServerConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(AppState::new(config.clone()));

    // Load persisted sessions from disk
    let cwd = config
        .cwd
        .as_deref()
        .map(Path::new)
        .unwrap_or_else(|| Path::new("."));
    state.load_persisted_sessions(cwd).await;

    let app = create_app(state);

    axum::serve(listener, app).await?;

    Ok(())
}

pub async fn serve_with_state(
    listener: TcpListener,
    state: Arc<AppState>,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = create_app(state);
    axum::serve(listener, app).await?;
    Ok(())
}
