//! Compaction types

use crate::session::entries::SessionEntry;

/// Compaction settings
#[derive(Debug, Clone)]
pub struct CompactionSettings {
    pub enabled: bool,
    pub reserve_tokens: u64,
    pub keep_recent_tokens: u64,
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

/// Result of a compaction operation
#[derive(Debug, Clone)]
pub struct CompactionResult<T = ()> {
    pub summary: String,
    pub first_kept_entry_id: String,
    pub tokens_before: u64,
    /// Extension-specific data
    pub details: Option<T>,
}

/// Cut point result
#[derive(Debug, Clone)]
pub struct CutPointResult {
    pub first_kept_entry_index: usize,
    pub turn_start_index: isize,
    pub is_split_turn: bool,
}

/// Context usage estimate
#[derive(Debug, Clone)]
pub struct ContextUsageEstimate {
    pub tokens: u64,
    pub usage_tokens: u64,
    pub trailing_tokens: u64,
    pub last_usage_index: Option<usize>,
}

/// Compaction preparation
#[derive(Debug, Clone)]
pub struct CompactionPreparation {
    pub first_kept_entry_id: String,
    pub messages_to_summarize: Vec<SessionEntry>,
    pub turn_prefix_messages: Vec<SessionEntry>,
    pub is_split_turn: bool,
    pub tokens_before: u64,
    pub previous_summary: Option<String>,
    pub file_ops: FileOperations,
    pub settings: CompactionSettings,
}

/// File operations tracked during session
#[derive(Debug, Clone, Default)]
pub struct FileOperations {
    pub read: Vec<String>,
    pub written: Vec<String>,
    pub edited: Vec<String>,
}

impl FileOperations {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get list of modified files (edited + written)
    pub fn modified_files(&self) -> Vec<String> {
        let mut files: Vec<String> = Vec::new();
        files.extend(self.edited.clone());
        files.extend(self.written.clone());
        files.sort();
        files.dedup();
        files
    }

    /// Get list of read-only files
    pub fn read_files(&self) -> Vec<String> {
        let modified = self.modified_files();
        self.read
            .iter()
            .filter(|f| !modified.contains(f))
            .cloned()
            .collect()
    }
}
