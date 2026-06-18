//! Bash command execution display component


use crate::core::tools::render_utils::ToolTheme;

/// Render bash execution display with command, output, and status
pub fn render_bash_execution(
    command: &str,
    output_lines: &[String],
    status: &str,
    exit_code: Option<i32>,
    expanded: bool,
    max_preview_lines: usize,
    truncated: bool,
    full_output_path: Option<&str>,
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let border = ToolTheme::fg("dim", &"─".repeat(std::cmp::max(1, width)));
    lines.push(border.clone());

    // Command header
    lines.push(ToolTheme::fg("accent", &format!("\x1b[1m$ {}\x1b[22m", command)));

    // Output
    if !output_lines.is_empty() {
        let display_lines = if expanded {
            output_lines.to_vec()
        } else {
            let start = if output_lines.len() > max_preview_lines {
                output_lines.len() - max_preview_lines
            } else {
                0
            };
            output_lines[start..].to_vec()
        };

        let hidden_count = output_lines.len() - display_lines.len();
        for line in &display_lines {
            lines.push(ToolTheme::fg("muted", line));
        }

        if hidden_count > 0 && !expanded {
            lines.push(ToolTheme::fg("muted", &format!("... {} more lines", hidden_count)));
        }
    }

    // Status
    let status_line = match status {
        "running" => ToolTheme::fg("accent", "Running..."),
        "cancelled" => ToolTheme::fg("warning", "(cancelled)"),
        "error" => ToolTheme::fg("error", &format!("(exit {})", exit_code.unwrap_or(-1))),
        "complete" => ToolTheme::fg("success", "(completed)"),
        _ => String::new(),
    };
    if !status_line.is_empty() {
        lines.push(status_line);
    }

    // Truncation info
    if truncated {
        if let Some(path) = full_output_path {
            lines.push(ToolTheme::fg("warning", &format!("Output truncated. Full output: {}", path)));
        } else {
            lines.push(ToolTheme::fg("warning", "Output truncated."));
        }
    }

    lines.push(border);
    lines
}
