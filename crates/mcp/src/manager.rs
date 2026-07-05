use std::sync::Arc;
use std::time::Duration;

use futures::future::join_all;
use pick_agent::core::state::AgentTool;
use tokio::sync::Mutex;

use crate::client::{McpToolExecutor, build_agent_tools, connect_and_discover};
use crate::config::McpServerConfig;

/// Information about a connected server, for display in /mcp list
#[derive(Debug, Clone)]
pub struct ConnectedServerInfo {
    pub name: String,
    pub transport: String,
    pub tool_count: usize,
    pub tool_names: Vec<String>,
    pub prompt_count: usize,
    pub prompt_names: Vec<String>,
    pub resource_count: usize,
    pub resource_names: Vec<String>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub url: Option<String>,
    pub is_connected: bool,
}

/// Runtime manager for MCP server connections.
/// Wraps the executor and provides connect/disconnect/list operations.
pub struct McpManager {
    executor: Arc<Mutex<McpToolExecutor>>,
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            executor: Arc::new(Mutex::new(McpToolExecutor::new())),
        }
    }

    /// Get the shared executor reference for tool dispatch.
    pub fn executor(&self) -> Arc<Mutex<McpToolExecutor>> {
        self.executor.clone()
    }

    /// Connect all servers from config in parallel.
    /// Returns the list of discovered AgentTools to add to the global tool list.
    pub async fn connect_from_config(&self, configs: &[McpServerConfig]) -> Vec<AgentTool> {
        let handles: Vec<_> = configs
            .iter()
            .map(|config| {
                let config = config.clone();
                let executor = self.executor.clone();
                let name = config.name.clone();
                tokio::spawn(async move {
                    match tokio::time::timeout(
                        Duration::from_secs(30),
                        connect_and_discover(&config),
                    )
                    .await
                    {
                        Ok(Ok(client)) => {
                            let entries = client.tools.clone();
                            let tools = build_agent_tools(entries, &executor);
                            let mut ex = executor.lock().await;
                            ex.add_client(client);
                            tools
                        }
                        Ok(Err(e)) => {
                            tracing::warn!("MCP server '{}' connection failed: {}", name, e);
                            Vec::new()
                        }
                        Err(_) => {
                            tracing::warn!("MCP server '{}' connection timed out after 30s", name);
                            Vec::new()
                        }
                    }
                })
            })
            .collect();

        join_all(handles)
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .flatten()
            .collect()
    }

    /// Get all MCP tool prefixed names from all connected servers.
    pub async fn get_all_mcp_tool_names(&self) -> Vec<String> {
        self.executor.lock().await.get_all_tool_names()
    }

    /// Gracefully shut down all MCP connections.
    /// Drops all clients, closing transports and killing child processes.
    pub async fn shutdown(&self) {
        let mut ex = self.executor.lock().await;
        ex.clear();
    }

    /// Connect a new MCP server at runtime.
    /// Returns the discovered AgentTools, or an error if the name already exists.
    pub async fn connect_server(&self, config: McpServerConfig) -> Result<Vec<AgentTool>, String> {
        let client = connect_and_discover(&config).await?;
        let entries = client.tools.clone();
        let tools = build_agent_tools(entries, &self.executor);

        let mut ex = self.executor.lock().await;
        ex.add_client(client);

        Ok(tools)
    }

    /// Disconnect an MCP server at runtime.
    /// Returns the names of removed tools, or an error if not found.
    pub async fn disconnect_server(&self, name: &str) -> Result<Vec<String>, String> {
        let mut ex = self.executor.lock().await;
        // Find and remove the client
        let pos = {
            // Need to iterate to find the position
            let mut found = None;
            for (i, c) in ex.clients().iter().enumerate() {
                if c.config.name == name {
                    found = Some(i);
                    break;
                }
            }
            found.ok_or_else(|| format!("Server '{}' not found", name))?
        };

        let removed = ex.remove_client(pos);
        let tool_names: Vec<String> = removed
            .tools
            .iter()
            .map(|t| t.prefixed_name.clone())
            .collect();

        Ok(tool_names)
    }

    /// List all connected servers
    pub async fn list_connections(&self) -> Vec<ConnectedServerInfo> {
        let ex = self.executor.lock().await;
        ex.clients().iter().map(build_connected_info).collect()
    }

    /// Get info for all servers (both connected and configured-but-disconnected).
    /// `configs` is the list of configured server configs (from settings).
    /// For servers that are connected, full capability info is returned.
    /// For servers that are not connected, only config info is returned.
    pub async fn get_all_servers_info(
        &self,
        configs: &[McpServerConfig],
    ) -> Vec<ConnectedServerInfo> {
        let ex = self.executor.lock().await;
        let mut result: Vec<ConnectedServerInfo> = Vec::new();

        // Collect names of connected servers
        let connected_names: std::collections::HashSet<String> =
            ex.clients().iter().map(|c| c.config.name.clone()).collect();

        // Add connected servers with full info
        for client in ex.clients() {
            result.push(build_connected_info(client));
        }

        // Add configured but disconnected servers (from settings)
        for config in configs {
            if !connected_names.contains(&config.name) {
                result.push(ConnectedServerInfo {
                    name: config.name.clone(),
                    transport: config.transport_type().to_string(),
                    tool_count: 0,
                    tool_names: Vec::new(),
                    prompt_count: 0,
                    prompt_names: Vec::new(),
                    resource_count: 0,
                    resource_names: Vec::new(),
                    command: config.command.clone(),
                    args: config.args.clone(),
                    url: config.url.clone(),
                    is_connected: false,
                });
            }
        }

        result
    }

    /// Check if a server is currently connected
    pub async fn is_server_connected(&self, name: &str) -> bool {
        let ex = self.executor.lock().await;
        ex.clients().iter().any(|c| c.config.name == name)
    }
}

fn build_connected_info(c: &crate::client::ConnectedClient) -> ConnectedServerInfo {
    let tool_names: Vec<String> = c.tools.iter().map(|t| t.prefixed_name.clone()).collect();
    let prompt_names: Vec<String> = c.prompts.iter().map(|p| p.name.clone()).collect();
    let resource_names: Vec<String> = c.resources.iter().map(|r| r.name.clone()).collect();
    ConnectedServerInfo {
        name: c.config.name.clone(),
        transport: c.config.transport_type().to_string(),
        tool_count: c.tools.len(),
        tool_names,
        prompt_count: c.prompts.len(),
        prompt_names,
        resource_count: c.resources.len(),
        resource_names,
        command: c.config.command.clone(),
        args: c.config.args.clone(),
        url: c.config.url.clone(),
        is_connected: true,
    }
}

/// Build a one-line description of a server config for slash command display
pub fn describe_config(config: &McpServerConfig) -> String {
    if let Some(url) = &config.url {
        format!("{} (http)", url)
    } else if let Some(cmd) = &config.command {
        let args_str = config
            .args
            .as_ref()
            .map(|a| a.join(" "))
            .unwrap_or_default();
        format!("{} {} (stdio)", cmd, args_str)
    } else {
        "unknown".to_string()
    }
}
