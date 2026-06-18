//! Diff rendering for tool results


use crate::core::tools::edit_diff::generate_diff_string;
use crate::core::tools::render_utils::ToolTheme;

/// Options for rendering a diff
pub struct RenderDiffOptions {
    pub context_lines: usize,
}

impl Default for RenderDiffOptions {
    fn default() -> Self {
        Self { context_lines: 3 }
    }
}

/// Render a diff between old and new content
pub fn render_diff(old_content: &str, new_content: &str, options: &RenderDiffOptions) -> String {
    let result = generate_diff_string(old_content, new_content, options.context_lines);
    let mut output = String::new();

    for line in result.diff.lines() {
        if line.starts_with('+') {
            output.push_str(&ToolTheme::fg("success", line));
        } else if line.starts_with('-') {
            output.push_str(&ToolTheme::fg("error", line));
        } else {
            output.push_str(line);
        }
        output.push('\n');
    }

    output
}
