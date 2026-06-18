//! Scoped models selector for enabling/disabling models


use crate::core::tools::render_utils::ToolTheme;

/// A scoped model item
#[derive(Clone)]
pub struct ScopedModelItem {
    pub full_id: String,
    pub provider: String,
    pub id: String,
    pub name: String,
    pub enabled: bool,
}

/// Render scoped models selector
pub fn render_scoped_models_selector(
    items: &[ScopedModelItem],
    selected_index: usize,
    search_query: &str,
    enabled_ids: Option<&[String]>,
    all_ids_len: usize,
    is_dirty: bool,
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));

    lines.push(ToolTheme::fg("accent", &border));
    lines.push(String::new());

    // Header
    lines.push(ToolTheme::fg("accent", &format!("\x1b[1m{}\x1b[22m", "Model Configuration")));
    lines.push(ToolTheme::fg("muted", "Session-only. Ctrl+S to save to settings."));
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
    let max_visible = 8;
    let total = items.len();
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
    let all_enabled = enabled_ids.is_none();

    for i in start..end {
        if let Some(item) = items.get(i) {
            let is_selected = i == selected_index;

            let prefix = if is_selected {
                ToolTheme::fg("accent", "→ ")
            } else {
                "  ".to_string()
            };
            let model_text = if is_selected {
                ToolTheme::fg("accent", &item.id)
            } else {
                item.id.clone()
            };
            let provider_badge = ToolTheme::fg("muted", &format!(" [{}]", item.provider));
            let status = if all_enabled {
                String::new()
            } else if item.enabled {
                ToolTheme::fg("success", " ✓")
            } else {
                ToolTheme::fg("dim", " ✗")
            };

            lines.push(format!("{}{}{}{}", prefix, model_text, provider_badge, status));
        }
    }

    if total > max_visible {
        lines.push(ToolTheme::fg("muted", &format!("  ({}/{})", selected_index + 1, total)));
    }

    // Model info
    if !items.is_empty() {
        if let Some(item) = items.get(selected_index) {
            lines.push(String::new());
            lines.push(ToolTheme::fg("muted", &format!("  Model Name: {}", item.name)));
        }
    } else {
        lines.push(ToolTheme::fg("muted", "  No matching models"));
    }

    // Footer
    lines.push(String::new());
    let enabled_count = enabled_ids.map(|ids| ids.len()).unwrap_or(all_ids_len);
    let count_text = if all_enabled {
        "all enabled"
    } else {
        &format!("{}/{} enabled", enabled_count, all_ids_len)
    };
    let footer = format!(
        "  Enter: toggle · Ctrl+A: all · Ctrl+C: clear · Ctrl+P: provider · Ctrl+↑/↓: reorder · Ctrl+S: save · {}",
        count_text
    );
    if is_dirty {
        lines.push(format!("{} {}", ToolTheme::fg("dim", &footer), ToolTheme::fg("warning", "(unsaved)")));
    } else {
        lines.push(ToolTheme::fg("dim", &footer));
    }

    lines.push(String::new());
    lines.push(ToolTheme::fg("accent", &border));
    lines
}
