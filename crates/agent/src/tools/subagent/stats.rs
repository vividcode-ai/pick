//! Subagent statistics types

/// Usage statistics tracked during in-process execution
#[derive(Debug, Clone, Default)]
pub struct SubagentStats {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub model: Option<String>,
    pub stop_reason: Option<String>,
    pub error_message: Option<String>,
    pub turns: u32,
}

/// Result from a single subagent execution
#[derive(Debug, Clone, Default)]
pub struct SingleResult {
    pub agent: String,
    pub exit_code: i32,
    pub output: String,
    pub error: String,
    pub stats: SubagentStats,
}
