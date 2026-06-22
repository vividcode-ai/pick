//! Model selector component with search

use crate::core::tools::render_utils::ToolTheme;

/// A model item for display
#[derive(Clone)]
pub struct ModelItem {
    pub provider: String,
    pub id: String,
    pub name: String,
    pub is_current: bool,
}

/// Render model selector with search and scope toggle
pub fn render_model_selector(
    models: &[ModelItem],
    selected_index: usize,
    scope: &str,
    has_scoped_models: bool,
    search_query: &str,
    error_message: Option<&str>,
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));

    lines.push(ToolTheme::fg("accent", &border));
    lines.push(String::new());

    // Scope toggle line
    if has_scoped_models {
        let all_text = if scope == "all" {
            ToolTheme::fg("accent", "all")
        } else {
            ToolTheme::fg("muted", "all")
        };
        let scoped_text = if scope == "scoped" {
            ToolTheme::fg("accent", "scoped")
        } else {
            ToolTheme::fg("muted", "scoped")
        };
        lines.push(format!(
            "{}Scope: {}{}{}",
            ToolTheme::fg("muted", ""),
            all_text,
            ToolTheme::fg("muted", " | "),
            scoped_text
        ));
        lines.push(ToolTheme::fg("dim", "  Tab: scope (all/scoped)"));
    } else {
        lines.push(ToolTheme::fg(
            "warning",
            "Only showing models from configured providers. Use /connect to add providers.",
        ));
    }
    lines.push(String::new());

    // Search input
    let search_display = if search_query.is_empty() {
        ToolTheme::fg("muted", "  Type to search...")
    } else {
        format!("  {}", search_query)
    };
    lines.push(search_display);
    lines.push(String::new());

    // Model list
    let max_visible = 10;
    let total = models.len();
    let start = if total > max_visible {
        let half = max_visible / 2;
        if selected_index > half {
            std::cmp::min(selected_index - half, total - max_visible)
        } else {
            0
        }
    } else {
        0
    };
    let end = std::cmp::min(start + max_visible, total);

    for i in start..end {
        if let Some(model) = models.get(i) {
            let is_selected = i == selected_index;

            if is_selected {
                let prefix = ToolTheme::fg("accent", "→ ");
                let model_text = ToolTheme::fg("accent", &model.id);
                let provider_badge = ToolTheme::fg("muted", &format!("[{}]", model.provider));
                let check = if model.is_current {
                    format!(" {}", ToolTheme::fg("success", "✓"))
                } else {
                    String::new()
                };
                lines.push(format!(
                    "{}{} {}{}",
                    prefix, model_text, provider_badge, check
                ));
            } else {
                let provider_badge = ToolTheme::fg("muted", &format!("[{}]", model.provider));
                let check = if model.is_current {
                    format!(" {}", ToolTheme::fg("success", "✓"))
                } else {
                    String::new()
                };
                lines.push(format!("  {} {}{}", model.id, provider_badge, check));
            }
        }
    }

    if total > max_visible {
        lines.push(ToolTheme::fg(
            "muted",
            &format!("  ({}/{})", selected_index + 1, total),
        ));
    }

    // Error or extra info
    if let Some(err) = error_message {
        for err_line in err.lines() {
            lines.push(ToolTheme::fg("error", err_line));
        }
    } else if !models.is_empty() {
        if let Some(model) = models.get(selected_index) {
            lines.push(String::new());
            lines.push(ToolTheme::fg(
                "muted",
                &format!("  Model Name: {}", model.name),
            ));
        }
    } else {
        lines.push(ToolTheme::fg("muted", "  No matching models"));
    }

    lines.push(String::new());
    lines.push(ToolTheme::fg("accent", &border));
    lines
}
