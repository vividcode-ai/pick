//! Text component that truncates to fit viewport width

use crate::utils::{truncate_to_width, visible_width};

/// Text component that truncates to fit viewport width
pub struct TruncatedText {
    text: String,
    padding_x: usize,
    padding_y: usize,
}

impl TruncatedText {
    pub fn new(text: impl Into<String>, padding_x: usize, padding_y: usize) -> Self {
        Self {
            text: text.into(),
            padding_x,
            padding_y,
        }
    }

    pub fn invalidate(&self) {}

    pub fn render(&self, width: usize) -> Vec<String> {
        let mut result = Vec::new();
        let empty_line = " ".repeat(width);

        // Add vertical padding above
        for _ in 0..self.padding_y {
            result.push(empty_line.clone());
        }

        // Calculate available width after horizontal padding
        let available_width = std::cmp::max(1, width.saturating_sub(self.padding_x * 2));

        // Take only the first line (stop at newline)
        let single_line = match self.text.find('\n') {
            Some(idx) => &self.text[..idx],
            None => &self.text,
        };

        // Truncate text if needed
        let display_text = truncate_to_width(single_line, available_width);

        // Add horizontal padding
        let left_pad = " ".repeat(self.padding_x);
        let right_pad = " ".repeat(self.padding_x);
        let line_with_padding = format!("{}{}{}", left_pad, display_text, right_pad);

        // Pad line to exactly width characters
        let line_vis_width = visible_width(&line_with_padding);
        let padding_needed = width.saturating_sub(line_vis_width);
        let final_line = format!("{}{}", line_with_padding, " ".repeat(padding_needed));
        result.push(final_line);

        // Add vertical padding below
        for _ in 0..self.padding_y {
            result.push(empty_line.clone());
        }

        result
    }
}
