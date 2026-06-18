//! Visual truncation utilities


/// Result of a visual truncation operation
pub struct VisualTruncateResult {
    /// The truncated content (may include ANSI codes)
    pub content: String,
    /// Whether the content was truncated
    pub truncated: bool,
}

/// Truncate text to fit within a given number of visual lines at a given width.
/// Handles ANSI escape codes correctly by not counting them as visible characters.
pub fn truncate_to_visual_lines(text: &str, max_lines: usize, width: usize) -> VisualTruncateResult {
    if max_lines == 0 {
        return VisualTruncateResult { content: String::new(), truncated: true };
    }

    // Use the TUI crate's text wrapping utility
    let lines = pick_tui::utils::wrap_text_with_ansi(text, width);

    if lines.len() <= max_lines {
        return VisualTruncateResult { content: text.to_string(), truncated: false };
    }

    let truncated = lines[..max_lines].join("\n");
    VisualTruncateResult { content: truncated, truncated: true }
}
