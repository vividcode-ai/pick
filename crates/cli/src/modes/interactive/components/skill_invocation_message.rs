//! Skill invocation message display

use crate::core::tools::render_utils::ToolTheme;

/// Render a skill invocation message
pub fn render_skill_invocation(name: &str, description: Option<&str>) -> String {
    let mut output = ToolTheme::fg("toolTitle", &format!("[Skill: {}]", name));
    if let Some(desc) = description {
        output.push_str(&format!("\n{}", ToolTheme::fg("toolOutput", desc)));
    }
    output
}
