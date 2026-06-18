/// Wrap a tool definition into a generic tool interface for the core runtime
pub fn wrap_tool_definition(
    definition: crate::core::extensions::types::ToolDefinition,
) -> ToolWrapper {
    ToolWrapper { definition }
}

/// A wrapped tool definition ready for runtime execution
pub struct ToolWrapper {
    pub definition: crate::core::extensions::types::ToolDefinition,
}

/// Synthesize a minimal ToolDefinition from a plain tool
pub fn create_tool_definition_from_tool(
    tool: &ToolWrapper,
) -> crate::core::extensions::types::ToolDefinition {
    crate::core::extensions::types::ToolDefinition {
        name: tool.definition.name.clone(),
        label: tool.definition.label.clone(),
        description: tool.definition.description.clone(),
        parameters: tool.definition.parameters.clone(),
        prompt_snippet: tool.definition.prompt_snippet.clone(),
        prompt_guidelines: tool.definition.prompt_guidelines.clone(),
        render_shell: tool.definition.render_shell.clone(),
        execution_mode: tool.definition.execution_mode.clone(),
    }
}
