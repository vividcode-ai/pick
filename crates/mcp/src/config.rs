use std::collections::HashMap;

/// Authentication configuration for MCP HTTP transport
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum McpAuthConfig {
    #[serde(rename = "bearer")]
    Bearer { token: String },
    #[serde(rename = "oauth2")]
    OAuth2 {
        client_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        client_secret: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        scopes: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        auth_url: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        token_url: Option<String>,
    },
}

/// Configuration for a single MCP server
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpServerConfig {
    /// Server name identifier
    pub name: String,
    /// Stdio transport: command to run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Stdio transport: arguments for the command
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    /// Stdio transport: environment variables
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    /// HTTP transport: server URL (mutually exclusive with command)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Optional prefix for all tool names from this server (to avoid name collisions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name_prefix: Option<String>,
    /// Authentication config for HTTP transport
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<McpAuthConfig>,
}

impl McpServerConfig {
    pub fn transport_type(&self) -> &str {
        if self.url.is_some() {
            "streamable-http"
        } else {
            "stdio"
        }
    }
}

/// Parse MCP server configs from settings JSON map (server name -> config object).
pub fn parse_mcp_configs_from_value(value: &serde_json::Value) -> Vec<McpServerConfig> {
    let Some(obj) = value.as_object() else {
        return Vec::new();
    };

    let mut configs = Vec::new();
    for (name, cfg) in obj {
        let Some(cfg_obj) = cfg.as_object() else {
            continue;
        };

        let command = cfg_obj
            .get("command")
            .and_then(|v| v.as_str())
            .map(String::from);
        let args = cfg_obj.get("args").and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });
        let env = cfg_obj.get("env").and_then(|v| v.as_object()).map(|o| {
            o.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                .collect()
        });
        let url = cfg_obj
            .get("url")
            .and_then(|v| v.as_str())
            .map(String::from);
        let tool_name_prefix = cfg_obj
            .get("tool_name_prefix")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Parse auth config
        let auth = cfg_obj
            .get("auth")
            .and_then(|v| serde_json::from_value::<McpAuthConfig>(v.clone()).ok());

        if command.is_none() && url.is_none() {
            continue;
        }

        configs.push(McpServerConfig {
            name: name.clone(),
            command,
            args,
            env,
            url,
            tool_name_prefix,
            auth,
        });
    }

    configs
}
