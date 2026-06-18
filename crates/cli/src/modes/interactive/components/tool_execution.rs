//! Tool execution display component


/// State of a tool execution in the display
pub enum ToolExecutionState {
    /// Tool call arguments received, not yet started
    Pending,
    /// Tool is currently executing
    Running,
    /// Tool completed successfully
    Success,
    /// Tool completed with an error
    Error,
}

/// Data for rendering a tool execution
pub struct ToolExecutionData {
    pub tool_name: String,
    pub args: serde_json::Value,
    pub state: ToolExecutionState,
    pub result_text: Option<String>,
    pub is_partial: bool,
}
