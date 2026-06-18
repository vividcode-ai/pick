//! Tree node data for TUI tree rendering

use crate::session::entries::SessionEntry;

/// Tree node data for TUI tree rendering.
/// Contains owned data so the TUI can hold it without lifetime constraints.
#[derive(Debug, Clone)]
pub struct TreeNodeData {
    pub entry_id: String,
    pub parent_id: Option<String>,
    pub depth: usize,
    pub has_children: bool,
    pub is_last: bool,
    /// Per-level gutters (true = show `│` at that level)
    pub gutters: Vec<bool>,
    pub label: Option<String>,
    pub label_timestamp: Option<String>,
    pub entry: SessionEntry,
}
