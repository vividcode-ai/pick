pub mod events;
pub mod rest;
pub mod session;
pub mod ws;

use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use pick_agent::session::GoalManager;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tracing::info;

pub struct AppState {
    pub session_manager: session::SessionManager,
    pub config: ServerConfig,
}

impl AppState {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            session_manager: session::SessionManager::new(),
            config,
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
        use std::sync::Arc;
        let gm = Arc::new(GoalManager::new());
        let agent_mode: Option<String> = None;
        registry::create_coding_tools_with_goal_manager(agent_mode, gm)
    }
}

#[derive(Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
        }
    }
}

pub async fn run_server(config: ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(AppState::new(config.clone()));

    let app = Router::new()
        .route("/health", get(rest::health))
        .route(
            "/sessions",
            get(rest::list_sessions).post(rest::create_session),
        )
        .route(
            "/sessions/{id}",
            get(rest::get_session).delete(rest::delete_session),
        )
        .route("/providers", get(rest::list_providers))
        .route("/ws", get(ws::handle_ws))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("{}:{}", config.host, config.port);
    info!("pick-server starting on {}", addr);

    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

pub async fn run_server_on_listener(
    listener: TcpListener,
    config: ServerConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(AppState::new(config));

    let app = Router::new()
        .route("/health", get(rest::health))
        .route(
            "/sessions",
            get(rest::list_sessions).post(rest::create_session),
        )
        .route(
            "/sessions/{id}",
            get(rest::get_session).delete(rest::delete_session),
        )
        .route("/providers", get(rest::list_providers))
        .route("/ws", get(ws::handle_ws))
        .layer(CorsLayer::permissive())
        .with_state(state);

    axum::serve(listener, app).await?;

    Ok(())
}
