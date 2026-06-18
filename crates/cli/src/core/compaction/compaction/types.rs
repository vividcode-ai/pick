use serde_json::Value;

use super::super::utils::FileOperations;

#[derive(Debug, Clone)]
pub struct CompactionDetails {
    pub read_files: Vec<String>,
    pub modified_files: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CompactionResult<T = serde_json::Value> {
    pub summary: String,
    pub first_kept_entry_id: String,
    pub tokens_before: usize,
    pub details: Option<T>,
}

#[derive(Debug, Clone)]
pub struct CompactionSettings {
    pub enabled: bool,
    pub reserve_tokens: usize,
    pub keep_recent_tokens: usize,
}

impl Default for CompactionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            reserve_tokens: 16384,
            keep_recent_tokens: 20000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContextUsageEstimate {
    pub tokens: usize,
    pub usage_tokens: usize,
    pub trailing_tokens: usize,
    pub last_usage_index: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct CutPointResult {
    pub first_kept_entry_index: usize,
    pub turn_start_index: Option<usize>,
    pub is_split_turn: bool,
}

#[derive(Debug, Clone)]
pub struct CompactionPreparation {
    pub first_kept_entry_id: String,
    pub messages_to_summarize: Vec<Value>,
    pub turn_prefix_messages: Vec<Value>,
    pub is_split_turn: bool,
    pub tokens_before: usize,
    pub previous_summary: Option<String>,
    pub file_ops: FileOperations,
    pub settings: CompactionSettings,
}

#[derive(Debug, Clone)]
pub struct CompactionError {
    pub code: String,
    pub message: String,
}
