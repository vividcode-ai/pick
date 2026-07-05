pub mod approval;
pub mod docs;
pub mod events;
pub mod files;
pub mod git;
pub mod mcp_routes;
pub mod plugins;
pub mod prompt_history_routes;
pub mod pty;
pub mod rest;
pub mod routes;
pub mod session;
pub mod settings_routes;
pub mod spa;
pub mod sse;

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::routing::{delete, get, post};
use pick_agent::prompt_history::PromptHistoryManager;
use pick_agent::session::manager::SessionManager as AgentSessionManager;
use pick_ai::types::Message;
use pick_mcp::McpManager;
use pick_mcp::config::McpServerConfig;
use pty::PtyManager;
use session::SseSessionState;
use tokio::net::TcpListener;
use tokio::sync::Mutex as TokioMutex;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tracing::debug;

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
    pub prompt_history: TokioMutex<PromptHistoryManager>,
}

impl AppState {
    pub fn new(config: ServerConfig) -> Self {
        let cwd = config.cwd.clone().unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
        let cwd_path = Path::new(&cwd);
        let session_dir = cwd_path.join(".pick").join("sessions");
        let history_cwd = cwd_path.to_path_buf();
        Self {
            session_manager: session::SessionManager::new_with_cwd(
                session_dir,
                cwd_path.to_path_buf(),
            ),
            sse_sessions: RwLock::new(HashMap::new()),
            config,
            default_provider: None,
            default_model: None,
            api_keys: HashMap::new(),
            pty_manager: PtyManager::new(Some(cwd)),
            mcp_manager: McpManager::new(),
            mcp_configs: RwLock::new(Vec::new()),
            prompt_history: TokioMutex::new(PromptHistoryManager::new(&history_cwd)),
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
        use pick_agent::session::GoalManager;
        use pick_agent::tools::registry;
        let gm = Arc::new(GoalManager::new());
        let agent_mode: Option<String> = None;
        registry::create_coding_tools_with_goal_manager(agent_mode, gm)
    }

    /// Load MCP servers from settings.json and connect them at startup
    pub async fn load_mcp_from_settings(&self, cwd: &std::path::Path) {
        use pick_agent::settings::{
            SettingsManager, get_global_settings_path, get_project_settings_path,
        };
        use pick_mcp::config::parse_mcp_configs_from_value;

        let sm = SettingsManager::load_from_paths(
            get_global_settings_path(),
            get_project_settings_path(cwd),
        );
        if let Some(servers) = &sm.get().mcp_servers {
            let disabled = sm.get_disabled_mcp_servers();
            let configs =
                parse_mcp_configs_from_value(&serde_json::to_value(servers).unwrap_or_default());
            let active: Vec<_> = configs
                .into_iter()
                .filter(|c| !disabled.contains(&c.name))
                .collect();
            if !active.is_empty() {
                let tools = self.mcp_manager.connect_from_config(&active).await;
                tracing::info!("Loaded {} MCP server(s) from settings", tools.len());
                let mut saved = self.mcp_configs.write().await;
                *saved = active;
            }
        }
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

    /// Load persisted sessions from disk using the agent's SessionManager
    pub async fn load_persisted_sessions(&self, cwd: &Path) {
        let dirs: Vec<std::path::PathBuf> = {
            let mut d = Vec::new();
            let project_dir = cwd.join(".pick").join("sessions");
            if project_dir.exists() {
                debug!("Loading session metadata from {}", project_dir.display());
                d.push(project_dir);
            }
            if let Some(home) = Self::home_dir() {
                let global_dir = home.join(".pick").join("agent").join("sessions");
                if global_dir.exists() {
                    debug!("Loading session metadata from {}", global_dir.display());
                    d.push(global_dir);
                }
            }
            d
        };

        for dir in dirs {
            let mut rd = match tokio::fs::read_dir(&dir).await {
                Ok(r) => r,
                Err(_) => continue,
            };
            loop {
                let entry = match rd.next_entry().await {
                    Ok(Some(e)) => e,
                    Ok(None) => break,
                    Err(_) => break,
                };
                if entry.path().extension() != Some(OsStr::new("jsonl")) {
                    continue;
                }
                match AgentSessionManager::open(entry.path(), cwd.to_path_buf()).await {
                    Ok(agent) => {
                        let id = agent.header().map(|h| h.id.clone()).unwrap_or_default();
                        let title = agent.get_session_name().unwrap_or("Session").to_string();
                        let model_id = agent
                            .header()
                            .and_then(|h| h.model.clone())
                            .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
                        let provider = agent
                            .header()
                            .and_then(|h| h.provider.clone())
                            .unwrap_or_else(|| "anthropic".to_string());
                        let thinking_level = agent
                            .header()
                            .and_then(|h| h.thinking_level.clone())
                            .unwrap_or_else(|| "off".to_string());
                        let messages: Vec<Message> = {
                            let leaf_id = match agent.get_leaf_id() {
                                Some(id) => id.to_string(),
                                None => continue,
                            };
                            agent
                                .get_path_to_root(&leaf_id)
                                .iter()
                                .filter_map(|e| Message::try_from(*e).ok())
                                .collect()
                        };
                        let msg_count = messages.len();
                        let path = agent
                            .session_path()
                            .map(|p| p.to_string_lossy().to_string());

                        let archived = agent.header().map(|h| h.archived).unwrap_or(false);

                        let session = session::AgentSession {
                            id,
                            title,
                            model_id,
                            provider,
                            thinking_level,
                            system_prompt: String::new(),
                            tools: Vec::new(),
                            messages,
                            created_at: agent.header().map(|h| h.created_at).unwrap_or_default(),
                            updated_at: agent.header().map(|h| h.updated_at).unwrap_or_default(),
                            status: "idle".to_string(),
                            fork_parent_id: None,
                            session_path: path,
                            persisted_messages_count: msg_count,
                            cwd: Some(cwd.to_string_lossy().to_string()),
                            archived,
                        };
                        self.session_manager.insert_session(session).await;
                        debug!("Loaded session meta: {}", entry.path().display());
                    }
                    Err(e) => {
                        debug!("Failed to load session {}: {}", entry.path().display(), e);
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub cwd: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            cwd: None,
        }
    }
}

pub fn create_app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(rest::health))
        .route("/server-config", get(rest::server_config))
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
        .route("/sessions/{id}/git-info", get(rest::get_session_git_info))
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
        .route("/pty-ws", get(pty::ws_handler))
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
        .route(
            "/settings",
            get(settings_routes::get_settings).patch(settings_routes::update_settings),
        )
        .route(
            "/prompt-history",
            get(prompt_history_routes::get_history_window),
        )
        .route(
            "/prompt-history/navigate",
            post(prompt_history_routes::navigate_history),
        )
        .route(
            "/prompt-history/push",
            post(prompt_history_routes::push_history),
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
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    state.load_persisted_sessions(&cwd).await;

    // Load MCP servers from settings in background (don't block startup)
    {
        let state_for_mcp = state.clone();
        let cwd_for_mcp = cwd.clone();
        tokio::spawn(async move {
            state_for_mcp.load_mcp_from_settings(&cwd_for_mcp).await;
        });
    }

    // Load prompt history into memory window
    {
        let mut history = state.prompt_history.lock().await;
        history.load();
    }

    let app = create_app(state.clone());

    let addr = format!("{}:{}", config.host, config.port);
    debug!("pick-server starting on {}", addr);

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
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    state.load_persisted_sessions(&cwd).await;

    // Load MCP servers from settings in background (don't block startup)
    {
        let state_for_mcp = state.clone();
        let cwd_for_mcp = cwd.clone();
        tokio::spawn(async move {
            state_for_mcp.load_mcp_from_settings(&cwd_for_mcp).await;
        });
    }

    // Load prompt history into memory window
    {
        let mut history = state.prompt_history.lock().await;
        history.load();
    }

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
