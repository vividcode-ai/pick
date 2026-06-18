//! Earendil announcement component

use crate::core::tools::render_utils::ToolTheme;

const BLOG_URL: &str = "https://mariozechner.at/posts/2026-04-08-ive-sold-out/";

/// Render the Earendil announcement
pub fn render_earendil_announcement(width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));
    lines.push(ToolTheme::fg("accent", &border));
    lines.push(ToolTheme::fg("accent", &format!("\x1b[1m{}\x1b[22m", "pick has joined Earendil")));
    lines.push(String::new());
    lines.push(ToolTheme::fg("muted", "Read the blog post:"));
    lines.push(ToolTheme::fg("accent", BLOG_URL));
    lines.push(String::new());
    lines.push(ToolTheme::fg("accent", &border));
    lines
}
