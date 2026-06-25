use std::sync::Arc;

use pick_agent::core::state::{AgentTool, AgentToolResult, ToolContext, ToolExecutionMode};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::{StreamableHttpClientTransport, TokioChildProcess};
use rmcp::{RoleClient, ServiceExt, model::CallToolRequestParams};
use tokio::sync::Mutex;

use crate::config::{McpAuthConfig, McpServerConfig};
use crate::conversion;

/// Built-in tool names that conflict with MCP tools would cause API errors.
const BUILTIN_TOOL_NAMES: &[&str] = &[
    "read",
    "write",
    "edit",
    "bash",
    "grep",
    "find",
    "ls",
    "subagent",
    "webfetch",
    "todo_plan",
    "question",
    "get_goal",
    "create_goal",
    "update_goal",
];

/// Type alias for the rmcp client handle
type McpClient = rmcp::service::RunningService<RoleClient, ()>;

/// Metadata for a discovered MCP tool
#[derive(Clone)]
pub struct ToolEntry {
    pub prefixed_name: String,
    pub original_name: String,
    pub description: String,
    pub input_schema: serde_json::Map<String, serde_json::Value>,
    #[allow(dead_code)]
    pub server_name: String,
}

/// A connected MCP server client with its discovered tools
pub struct ConnectedClient {
    pub client: Arc<Mutex<McpClient>>,
    pub tools: Vec<ToolEntry>,
    pub config: McpServerConfig,
}

/// Shared executor holding all MCP client connections.
pub struct McpToolExecutor {
    clients: Vec<ConnectedClient>,
}

impl Default for McpToolExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl McpToolExecutor {
    pub fn new() -> Self {
        Self {
            clients: Vec::new(),
        }
    }

    /// Add a connected client and its tools to the executor
    pub fn add_client(&mut self, client: ConnectedClient) {
        self.clients.push(client);
    }

    /// Get a reference to all connected clients
    pub fn clients(&self) -> &[ConnectedClient] {
        &self.clients
    }

    /// Remove and return a client by index
    pub fn remove_client(&mut self, index: usize) -> ConnectedClient {
        self.clients.remove(index)
    }

    /// Clear all clients, dropping them and closing all connections
    pub fn clear(&mut self) {
        self.clients.clear();
    }

    /// Execute an MCP tool by its prefixed name
    pub async fn call_tool(
        &self,
        prefixed_name: &str,
        args: serde_json::Value,
    ) -> Result<AgentToolResult, String> {
        let (client, entry) = self.find_tool(prefixed_name).ok_or_else(|| {
            format!(
                "MCP tool '{}' not found on any connected server",
                prefixed_name
            )
        })?;

        let args_map = match args {
            serde_json::Value::Object(map) => map,
            other => {
                let mut map = serde_json::Map::new();
                map.insert("value".to_string(), other);
                map
            }
        };

        let params =
            CallToolRequestParams::new(entry.original_name.clone()).with_arguments(args_map);

        let client = client.lock().await;
        let result = client
            .call_tool(params)
            .await
            .map_err(|e| format!("MCP call_tool '{}' failed: {}", prefixed_name, e))?;

        let is_error = result.is_error.unwrap_or(false);
        Ok(conversion::mcp_result_to_agent_result(
            is_error,
            &result.content,
        ))
    }

    /// Get all MCP tool prefixed names from all connected servers
    pub fn get_all_tool_names(&self) -> Vec<String> {
        self.clients
            .iter()
            .flat_map(|c| c.tools.iter().map(|t| t.prefixed_name.clone()))
            .collect()
    }

    fn find_tool(&self, prefixed_name: &str) -> Option<(&Arc<Mutex<McpClient>>, &ToolEntry)> {
        for ct in &self.clients {
            for tool in &ct.tools {
                if tool.prefixed_name == prefixed_name {
                    return Some((&ct.client, tool));
                }
            }
        }
        None
    }
}

/// Create AgentTools from a list of tool entries, referencing a shared executor.
pub fn build_agent_tools(
    entries: Vec<ToolEntry>,
    executor: &Arc<Mutex<McpToolExecutor>>,
) -> Vec<AgentTool> {
    let executor_ref = executor.clone();
    entries
        .into_iter()
        .map(|entry| {
            let prefixed_name = entry.prefixed_name.clone();
            let description = entry.description.clone();
            let prompt_snippet =
                conversion::generate_prompt_snippet(&prefixed_name, &entry.input_schema);
            let parameters = conversion::json_schema_from_mcp(&entry.input_schema);
            let ex = executor_ref.clone();
            let name_for_closure = prefixed_name.clone();

            AgentTool {
                name: prefixed_name,
                description,
                prompt_snippet: Some(prompt_snippet),
                prompt_guidelines: Vec::new(),
                label: name_for_closure.clone(),
                parameters,
                execute: Arc::new(move |_tool_call_id, args, _ctx: ToolContext| {
                    let ex = ex.clone();
                    let name = name_for_closure.clone();
                    Box::pin(async move {
                        let ex = ex.lock().await;
                        ex.call_tool(&name, args).await
                    })
                }),
                execution_mode: ToolExecutionMode::Sequential,
            }
        })
        .collect()
}

/// Connect to an MCP server via stdio or HTTP, discover tools,
/// and return a ConnectedClient.
pub async fn connect_and_discover(config: &McpServerConfig) -> Result<ConnectedClient, String> {
    let client = if config.url.is_some() {
        connect_http(config).await?
    } else {
        connect_stdio(config).await?
    };

    // Discover tools
    let tools = client
        .list_all_tools()
        .await
        .map_err(|e| format!("Failed to list tools from '{}': {}", config.name, e))?;

    // Auto-detect tool name conflicts and apply prefix if needed
    let prefix = config.tool_name_prefix.clone().unwrap_or_else(|| {
        let has_conflict = tools
            .iter()
            .any(|t| BUILTIN_TOOL_NAMES.contains(&t.name.as_ref()));
        if has_conflict {
            let default_prefix = format!("{}_", config.name);
            default_prefix
        } else {
            String::new()
        }
    });
    let server_name = config.name.clone();

    let entries: Vec<ToolEntry> = tools
        .into_iter()
        .map(|tool| {
            let prefixed_name = format!("{}{}", prefix, tool.name);
            ToolEntry {
                prefixed_name,
                original_name: tool.name.to_string(),
                description: tool.description.as_deref().unwrap_or("").to_string(),
                input_schema: (*tool.input_schema).clone(),
                server_name: server_name.clone(),
            }
        })
        .collect();

    Ok(ConnectedClient {
        client: Arc::new(Mutex::new(client)),
        tools: entries,
        config: config.clone(),
    })
}

/// Connect via stdio transport
async fn connect_stdio(config: &McpServerConfig) -> Result<McpClient, String> {
    let command = config
        .command
        .as_ref()
        .ok_or_else(|| format!("No command for server '{}'", config.name))?;

    let mk_cmd = |cmd_name: &str, original_cmd: &str, original_args: &[String]| {
        let mut cmd = tokio::process::Command::new(cmd_name);
        if cmd_name == original_cmd {
            cmd.args(original_args);
        } else {
            // cmd /c wrapper: pass original command + args as a single /c argument
            let mut full_args = vec![original_cmd.to_string()];
            full_args.extend(original_args.iter().cloned());
            cmd.arg("/c").arg(full_args.join(" "));
        }
        // Apply environment variables (RUST_LOG, etc.)
        if let Some(ref env) = config.env {
            for (k, v) in env {
                cmd.env(k, v);
            }
        }
        cmd
    };

    let args = config.args.as_deref().unwrap_or(&[]);
    let args_owned: Vec<String> = args.to_vec();

    /// Spawn an MCP child process with piped stdio and null stderr to prevent
    /// child process log output from leaking to the terminal (rmcp's default
    /// builder sets stderr to inherit, which would pollute the TUI).
    fn spawn_mcp(cmd: tokio::process::Command) -> Result<TokioChildProcess, std::io::Error> {
        let (child, _stderr) = TokioChildProcess::builder(cmd)
            .stderr(std::process::Stdio::null())
            .spawn()?;
        Ok(child)
    }

    // Try direct spawn first
    let transport = match spawn_mcp(mk_cmd(command, command, &args_owned)) {
        Ok(t) => t,
        Err(e) => {
            // On Windows, .cmd/.bat scripts fail with "program not found" via CreateProcess.
            // Retry with cmd /c wrapper.
            if cfg!(windows) {
                spawn_mcp(mk_cmd("cmd", command, &args_owned))
                    .map_err(|e2| format!("Failed to spawn MCP server '{}': {}", config.name, e2))?
            } else {
                return Err(format!(
                    "Failed to spawn MCP server '{}': {}",
                    config.name, e
                ));
            }
        }
    };

    let client = ()
        .serve(transport)
        .await
        .map_err(|e| format!("Failed to initialize MCP client '{}': {}", config.name, e))?;

    Ok(client)
}

/// Connect via Streamable HTTP transport
async fn connect_http(config: &McpServerConfig) -> Result<McpClient, String> {
    let url = config.url.as_ref().unwrap();

    let cfg = match &config.auth {
        Some(McpAuthConfig::Bearer { token }) => {
            StreamableHttpClientTransportConfig::with_uri(url.as_str()).auth_header(token.clone())
        }
        Some(McpAuthConfig::OAuth2 { .. }) => {
            return Err("OAuth2 not yet implemented for HTTP MCP transport".to_string());
        }
        None => StreamableHttpClientTransportConfig::with_uri(url.as_str()),
    };

    let transport = StreamableHttpClientTransport::from_config(cfg);

    let client = ().serve(transport).await.map_err(|e| {
        format!(
            "Failed to connect to MCP HTTP server '{}': {}",
            config.name, e
        )
    })?;

    Ok(client)
}
