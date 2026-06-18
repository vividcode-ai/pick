//! Compaction summary message display


use crate::core::tools::render_utils::ToolTheme;

/// Render a compaction summary message
pub fn render_compaction_summary(
    messages_compacted: usize,
    tokens_saved: u64,
) -> String {
    ToolTheme::fg("dim", &format!(
        "[Compaction: {} messages compacted, ~{} tokens saved]",
        messages_compacted, tokens_saved
    ))
}
