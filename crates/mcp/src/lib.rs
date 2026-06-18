//! MCP (Model Context Protocol) client integration for Pick.
//! Connects to MCP servers via stdio or HTTP transport, discovers tools,
//! and wraps them as Pick AgentTools for use in the agent loop.

pub mod config;
pub mod client;
pub mod conversion;
pub mod manager;

pub use config::{McpAuthConfig, McpServerConfig, parse_mcp_configs_from_value};
pub use client::{McpToolExecutor, build_agent_tools, connect_and_discover};
pub use manager::{McpManager, ConnectedServerInfo, describe_config};
