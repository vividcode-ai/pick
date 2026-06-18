//! Types and constants for the editor

use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

/// A pending paste with its placeholder text and the actual content.
/// Placeholders are inserted into the buffer; on submit they are
/// expanded back to the original pasted content.
#[derive(Debug, Clone)]
pub struct PendingPaste {
    pub placeholder: String,
    pub actual: String,
}

/// Split a string into chunks where each chunk's display width does not exceed `max_width`.
/// Uses Unicode display width (CJK chars = 2) to ensure proper character-boundary splitting.
pub fn wrap_by_display_width(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 || text.is_empty() {
        return vec![text.to_string()];
    }
    let mut result = Vec::new();
    let mut line_start = 0;
    for (i, c) in text.char_indices() {
        let c_width = UnicodeWidthChar::width(c).unwrap_or(0);
        let prefix_width = UnicodeWidthStr::width(&text[line_start..i]);
        if prefix_width + c_width > max_width && prefix_width > 0 {
            result.push(text[line_start..i].to_string());
            line_start = i;
        }
    }
    if line_start < text.len() {
        result.push(text[line_start..].to_string());
    }
    result
}
