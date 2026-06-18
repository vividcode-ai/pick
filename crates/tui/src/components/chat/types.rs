use serde_json;

pub const THINK_PREFIX: &str = "\x1b[3m\x1b[38;2;128;128;128m";
pub const THINK_SUFFIX: &str = "\x1b[23m\x1b[39m";
pub const TOOL_CALL_MAX_LINES: usize = 5;

#[allow(dead_code)]
pub const TOOL_OUTPUT_COLOR: &str = " \x1b[38;2;128;128;128m";
pub const MUTED_COLOR: &str = " \x1b[38;2;128;128;128m";

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolStatus {
    Pending,
    Success,
    Error,
}

#[derive(Debug, Clone)]
pub struct ToolExecutionEntry {
    pub tool_call_id: String,
    pub tool_name: String,
    pub args: serde_json::Value,
    pub status: ToolStatus,
    pub output: String,
    pub expanded: bool,
}

#[derive(Debug, Clone)]
pub enum ChatEntry {
    Message(ChatMessage),
    ToolExecution(ToolExecutionEntry),
}
