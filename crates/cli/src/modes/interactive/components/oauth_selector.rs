//! OAuth provider selector component

use crate::core::tools::render_utils::ToolTheme;

/// Auth provider info
#[derive(Clone)]
pub struct AuthProvider {
    pub id: String,
    pub name: String,
    pub auth_type: String,
    pub configured: bool,
    pub config_label: Option<String>,
}

/// Render OAuth provider selector
pub fn render_oauth_selector(
    mode: &str,
    providers: &[AuthProvider],
    selected_index: usize,
    search_query: &str,
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));

    lines.push(ToolTheme::fg("accent", &border));
    lines.push(String::new());

    let title = if mode == "login" {
        "Select provider to configure:"
    } else {
        "Select provider to logout:"
    };
    lines.push(ToolTheme::fg(
        "accent",
        &format!("\x1b[1m{}\x1b[22m", title),
    ));
    lines.push(String::new());

    // Search input
    let search_display = if search_query.is_empty() {
        ToolTheme::fg("muted", "  Type to search...")
    } else {
        format!("  {}", search_query)
    };
    lines.push(search_display);
    lines.push(String::new());

    // Provider list
    let max_visible = 8;
    let total = providers.len();
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
        if let Some(p) = providers.get(i) {
            let is_selected = i == selected_index;

            let status = if p.configured {
                if let Some(ref label) = p.config_label {
                    format!(" {}", ToolTheme::fg("success", label))
                } else {
                    format!(" {}", ToolTheme::fg("success", "✓ configured"))
                }
            } else {
                String::new()
            };

            if is_selected {
                let prefix = ToolTheme::fg("accent", "→ ");
                let name = ToolTheme::fg("accent", &p.name);
                lines.push(format!("{}{}{}", prefix, name, status));
            } else {
                lines.push(format!("  {}{}", p.name, status));
            }
        }
    }

    if total > max_visible {
        lines.push(ToolTheme::fg(
            "muted",
            &format!("  ({}/{})", selected_index + 1, total),
        ));
    }

    // Empty messages
    if providers.is_empty() {
        let msg = if total == 0 {
            if mode == "login" {
                "No providers available"
            } else {
                "No providers logged in. Use /connect first."
            }
        } else {
            "No matching providers"
        };
        lines.push(ToolTheme::fg("muted", &format!("  {}", msg)));
    }

    lines.push(String::new());
    lines.push(ToolTheme::fg("accent", &border));
    lines
}
