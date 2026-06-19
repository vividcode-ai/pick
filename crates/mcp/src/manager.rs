use std::sync::Arc;

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
                tokio::spawn(async move {
                    match connect_and_discover(&config).await {
                        Ok(client) => {
                            let entries = client.tools.clone();
                            let tools = build_agent_tools(entries, &executor);
                            let mut ex = executor.lock().await;
                            ex.add_client(client);
                            tools
                        }
                        Err(_) => Vec::new(),
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
        ex.clients()
            .iter()
            .map(|c| {
                let tool_names: Vec<String> =
                    c.tools.iter().map(|t| t.prefixed_name.clone()).collect();
                ConnectedServerInfo {
                    name: c.config.name.clone(),
                    transport: c.config.transport_type().to_string(),
                    tool_count: c.tools.len(),
                    tool_names,
                }
            })
            .collect()
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
