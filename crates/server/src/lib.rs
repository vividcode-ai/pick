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
use std::sync::Arc;

use axum::Router;
use axum::routing::{delete, get, post};
use pick_agent::session::GoalManager;
use pick_mcp::McpManager;
use pick_mcp::config::McpServerConfig;
use pty::PtyManager;
use session::SseSessionState;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tracing::info;

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
        Self {
            session_manager: session::SessionManager::new(),
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
}

#[derive(Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub pty_ws_port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            pty_ws_port: 9000,
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
    let state = Arc::new(AppState::new(config));
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
