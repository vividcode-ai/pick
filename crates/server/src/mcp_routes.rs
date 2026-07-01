use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use pick_mcp::config::McpServerConfig;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::AppState;

#[derive(Deserialize, ToSchema)]
pub struct AddMcpServerRequest {
    pub name: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub url: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct McpServerStatus {
    pub name: String,
    pub transport: String,
    pub tool_count: usize,
    pub tool_names: Vec<String>,
    pub prompt_count: usize,
    pub prompt_names: Vec<String>,
    pub resource_count: usize,
    pub resource_names: Vec<String>,
    pub is_connected: bool,
}

/// List all MCP servers (connected and configured)
#[utoipa::path(
    get,
    path = "/mcp",
    tag = "mcp",
    responses(
        (status = 200, description = "List of MCP servers", body = Vec<McpServerStatus>),
    )
)]
pub async fn list_mcp_servers(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let configs = state.mcp_configs.read().await;
    let info = state.mcp_manager.get_all_servers_info(&configs).await;
    let statuses: Vec<McpServerStatus> = info
        .into_iter()
        .map(|i| McpServerStatus {
            name: i.name,
            transport: i.transport,
            tool_count: i.tool_count,
            tool_names: i.tool_names,
            prompt_count: i.prompt_count,
            prompt_names: i.prompt_names,
            resource_count: i.resource_count,
            resource_names: i.resource_names,
            is_connected: i.is_connected,
        })
        .collect();
    Json(statuses)
}

/// Connect a new MCP server
#[utoipa::path(
    post,
    path = "/mcp",
    tag = "mcp",
    request_body = AddMcpServerRequest,
    responses(
        (status = 201, description = "MCP server connected"),
        (status = 400, description = "Failed to connect"),
    )
)]
pub async fn add_mcp_server(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddMcpServerRequest>,
) -> impl IntoResponse {
    let config = McpServerConfig {
        name: req.name.clone(),
        command: req.command,
        args: req.args,
        url: req.url,
        env: None,
        auth: None,
        tool_name_prefix: None,
    };

    match state.mcp_manager.connect_server(config.clone()).await {
        Ok(tools) => {
            // Save to config list
            let mut configs = state.mcp_configs.write().await;
            configs.push(config);
            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "name": req.name,
                    "tools_discovered": tools.len(),
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

/// Disconnect an MCP server
#[utoipa::path(
    delete,
    path = "/mcp/{name}",
    tag = "mcp",
    params(
        ("name" = String, Path, description = "MCP server name"),
    ),
    responses(
        (status = 204, description = "MCP server disconnected"),
        (status = 404, description = "Server not found"),
    )
)]
pub async fn remove_mcp_server(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    // Remove from config list
    {
        let mut configs = state.mcp_configs.write().await;
        configs.retain(|c| c.name != name);
    }

    match state.mcp_manager.disconnect_server(&name).await {
        Ok(removed_tools) => {
            if removed_tools.is_empty() {
                (
                    StatusCode::OK,
                    "Server was not connected, removed from config",
                )
                    .into_response()
            } else {
                (StatusCode::NO_CONTENT).into_response()
            }
        }
        Err(e) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": e}))).into_response(),
    }
}

/// Reconnect a configured MCP server
#[utoipa::path(
    post,
    path = "/mcp/{name}/reconnect",
    tag = "mcp",
    params(
        ("name" = String, Path, description = "MCP server name"),
    ),
    responses(
        (status = 200, description = "MCP server reconnected"),
        (status = 404, description = "Server config not found"),
    )
)]
pub async fn reconnect_mcp_server(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    // Find the config
    let config = {
        let configs = state.mcp_configs.read().await;
        configs.iter().find(|c| c.name == name).cloned()
    };

    match config {
        Some(cfg) => {
            // Disconnect first if connected
            let _ = state.mcp_manager.disconnect_server(&name).await;
            // Reconnect
            match state.mcp_manager.connect_server(cfg).await {
                Ok(tools) => (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "name": name,
                        "tools_discovered": tools.len(),
                    })),
                )
                    .into_response(),
                Err(e) => (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": e})),
                )
                    .into_response(),
            }
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("Server '{}' not found in config", name)})),
        )
            .into_response(),
    }
}
