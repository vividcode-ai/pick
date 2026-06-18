//! Tool wrappers for extension-registered tools

use super::types::{RegisteredTool, ToolDefinition};

/// Wrap a RegisteredTool into a format compatible with the tool system
pub fn wrap_registered_tool(registered_tool: &RegisteredTool) -> ToolDefinition {
    registered_tool.definition.clone()
}

/// Wrap all registered tools
pub fn wrap_registered_tools(registered_tools: &[RegisteredTool]) -> Vec<ToolDefinition> {
    registered_tools.iter().map(wrap_registered_tool).collect()
}
