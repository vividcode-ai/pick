use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::Serialize;
use utoipa::ToSchema;

use crate::AppState;

#[derive(Serialize, ToSchema)]
pub struct PluginInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub plugin_type: String,
    pub description: String,
    pub tool_count: usize,
    pub tools: Vec<String>,
    pub is_connected: bool,
}

/// List all plugins (extensions and MCP servers)
#[utoipa::path(
    get,
    path = "/plugins",
    responses(
        (status = 200, description = "List of all plugins", body = Vec<PluginInfo>),
    )
)]
pub async fn list_plugins(State(state): State<Arc<AppState>>) -> Json<Vec<PluginInfo>> {
    let mut plugins: Vec<PluginInfo> = Vec::new();

    // List MCP servers as plugins
    let mcp_configs = state.mcp_configs.read().await;
    let server_info = state.mcp_manager.get_all_servers_info(&mcp_configs).await;
    for info in server_info {
        plugins.push(PluginInfo {
            name: info.name.clone(),
            plugin_type: "mcp".to_string(),
            description: format!("MCP server via {} transport", info.transport),
            tool_count: info.tool_count,
            tools: info.tool_names.clone(),
            is_connected: info.is_connected,
        });
    }

    Json(plugins)
}
