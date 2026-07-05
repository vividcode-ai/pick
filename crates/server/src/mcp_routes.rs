use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use pick_agent::settings::{SettingsManager, get_global_settings_path, get_project_settings_path};
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
    pub env: Option<HashMap<String, String>>,
    pub tool_name_prefix: Option<String>,
    pub auth: Option<serde_json::Value>,
    pub scope: Option<String>,
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
    let auth = req
        .auth
        .as_ref()
        .and_then(|v| serde_json::from_value::<pick_mcp::config::McpAuthConfig>(v.clone()).ok());

    let config = McpServerConfig {
        name: req.name.clone(),
        command: req.command.clone(),
        args: req.args.clone(),
        url: req.url.clone(),
        env: req.env.clone(),
        auth,
        tool_name_prefix: req.tool_name_prefix.clone(),
    };

    match state.mcp_manager.connect_server(config.clone()).await {
        Ok(tools) => {
            // Save to in-memory config list
            {
                let mut configs = state.mcp_configs.write().await;
                configs.push(config);
            }

            // Persist to settings file
            persist_mcp_server_to_settings(
                &state,
                &req.name,
                &req.command,
                &req.args,
                &req.url,
                &req.env,
                &req.tool_name_prefix,
                &req.auth,
                req.scope.as_deref(),
            );

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

    // Remove from settings file
    remove_mcp_server_from_settings(&state, &name);

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

// ── Helpers: persist MCP server config to settings.json ──

fn get_cwd(state: &Arc<AppState>) -> &std::path::Path {
    state
        .config
        .cwd
        .as_deref()
        .map(std::path::Path::new)
        .unwrap_or_else(|| std::path::Path::new("."))
}

fn persist_mcp_server_to_settings(
    state: &Arc<AppState>,
    name: &str,
    command: &Option<String>,
    args: &Option<Vec<String>>,
    url: &Option<String>,
    env: &Option<HashMap<String, String>>,
    tool_name_prefix: &Option<String>,
    auth: &Option<serde_json::Value>,
    scope: Option<&str>,
) {
    let cwd = get_cwd(state);
    let mut sm = SettingsManager::load_from_paths(
        get_global_settings_path(),
        get_project_settings_path(cwd),
    );

    let server_entry = pick_agent::settings::McpServerConfigJson {
        command: command.clone(),
        args: args.clone(),
        env: env.clone(),
        url: url.clone(),
        tool_name_prefix: tool_name_prefix.clone(),
        auth: auth.clone(),
    };

    let current = sm.get().clone();
    let mut mcp_servers = current.mcp_servers.unwrap_or_default();
    mcp_servers.insert(name.to_string(), server_entry);

    let mut patch = pick_agent::settings::Settings::default();
    patch.mcp_servers = Some(mcp_servers);

    match scope {
        Some("project") => {
            let _ = sm.set_project(patch);
        }
        _ => {
            let _ = sm.set_global(patch);
        }
    }
}

fn remove_mcp_server_from_settings(state: &Arc<AppState>, name: &str) {
    let cwd = get_cwd(state);
    let mut sm = SettingsManager::load_from_paths(
        get_global_settings_path(),
        get_project_settings_path(cwd),
    );

    // Try global
    if sm
        .get_global()
        .mcp_servers
        .as_ref()
        .map_or(false, |s| s.contains_key(name))
    {
        let mut global = sm.get_global().clone();
        if let Some(ref mut servers) = global.mcp_servers {
            servers.remove(name);
        }
        let mut patch = pick_agent::settings::Settings::default();
        patch.mcp_servers = global.mcp_servers;
        let _ = sm.set_global(patch);
    }

    // Try project
    if sm
        .get_project()
        .mcp_servers
        .as_ref()
        .map_or(false, |s| s.contains_key(name))
    {
        let mut project = sm.get_project().clone();
        if let Some(ref mut servers) = project.mcp_servers {
            servers.remove(name);
        }
        let mut patch = pick_agent::settings::Settings::default();
        patch.mcp_servers = project.mcp_servers;
        let _ = sm.set_project(patch);
    }
}
