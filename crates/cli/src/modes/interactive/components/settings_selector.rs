//! Settings selector component with settings list and submenus

use crate::core::tools::render_utils::ToolTheme;

/// A settings item for display
#[derive(Clone)]
pub struct SettingDisplayItem {
    pub id: String,
    pub label: String,
    pub description: String,
    pub current_value: String,
    pub has_submenu: bool,
}

/// Thinking level descriptions
const THINKING_DESCRIPTIONS: &[(&str, &str)] = &[
    ("off", "No reasoning"),
    ("minimal", "Very brief reasoning (~1k tokens)"),
    ("low", "Light reasoning (~2k tokens)"),
    ("medium", "Moderate reasoning (~8k tokens)"),
    ("high", "Deep reasoning (~16k tokens)"),
    ("xhigh", "Maximum reasoning (~32k tokens)"),
];

/// Build the standard settings items list
pub fn build_settings_items(
    auto_compact: bool,
    show_images: bool,
    image_width_cells: u32,
    auto_resize_images: bool,
    block_images: bool,
    enable_skill_commands: bool,
    steering_mode: &str,
    follow_up_mode: &str,
    transport: &str,
    http_idle_timeout_ms: u64,
    thinking_level: &str,
    _available_thinking_levels: &[String],
    current_theme: &str,
    _available_themes: &[String],
    hide_thinking_block: bool,
    collapse_changelog: bool,
    enable_install_telemetry: bool,
    double_escape_action: &str,
    tree_filter_mode: &str,
    show_hardware_cursor: bool,
    editor_padding_x: u32,
    autocomplete_max_visible: u32,
    quiet_startup: bool,
    clear_on_shrink: bool,
    show_terminal_progress: bool,
) -> Vec<SettingDisplayItem> {
    vec![
        SettingDisplayItem {
            id: "autocompact".to_string(),
            label: "Auto-compact".to_string(),
            description: "Automatically compact context when it gets too large".to_string(),
            current_value: if auto_compact { "enabled" } else { "disabled" }.to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "show-images".to_string(),
            label: "Show images".to_string(),
            description: "Render images inline in terminal".to_string(),
            current_value: if show_images { "enabled" } else { "disabled" }.to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "image-width-cells".to_string(),
            label: "Image width".to_string(),
            description: "Preferred inline image width in terminal cells".to_string(),
            current_value: image_width_cells.to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "auto-resize-images".to_string(),
            label: "Auto-resize images".to_string(),
            description: "Resize large images to 2000x2000 max for better model compatibility"
                .to_string(),
            current_value: if auto_resize_images {
                "enabled"
            } else {
                "disabled"
            }
            .to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "block-images".to_string(),
            label: "Block images".to_string(),
            description: "Prevent images from being sent to LLM providers".to_string(),
            current_value: if block_images { "enabled" } else { "disabled" }.to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "skill-commands".to_string(),
            label: "Skill commands".to_string(),
            description: "Register skills as /skill:name commands".to_string(),
            current_value: if enable_skill_commands {
                "enabled"
            } else {
                "disabled"
            }
            .to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "steering-mode".to_string(),
            label: "Steering mode".to_string(),
            description: "Enter while streaming queues steering messages".to_string(),
            current_value: steering_mode.to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "follow-up-mode".to_string(),
            label: "Follow-up mode".to_string(),
            description: "Follow-up key queues follow-up messages until agent stops".to_string(),
            current_value: follow_up_mode.to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "transport".to_string(),
            label: "Transport".to_string(),
            description: "Preferred transport for providers that support multiple transports"
                .to_string(),
            current_value: transport.to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "http-idle-timeout".to_string(),
            label: "HTTP idle timeout".to_string(),
            description: "Maximum idle gap while waiting for HTTP headers or body chunks"
                .to_string(),
            current_value: format_http_idle_timeout(http_idle_timeout_ms),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "hide-thinking".to_string(),
            label: "Show thinking".to_string(),
            description: "Show thinking blocks in assistant responses".to_string(),
            current_value: if hide_thinking_block {
                "disabled"
            } else {
                "enabled"
            }
            .to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "collapse-changelog".to_string(),
            label: "Collapse changelog".to_string(),
            description: "Show condensed changelog after updates".to_string(),
            current_value: if collapse_changelog {
                "enabled"
            } else {
                "disabled"
            }
            .to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "quiet-startup".to_string(),
            label: "Quiet startup".to_string(),
            description: "Disable verbose printing at startup".to_string(),
            current_value: if quiet_startup { "enabled" } else { "disabled" }.to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "install-telemetry".to_string(),
            label: "Install telemetry".to_string(),
            description: "Send an anonymous version/update ping after changelog-detected updates"
                .to_string(),
            current_value: if enable_install_telemetry {
                "enabled"
            } else {
                "disabled"
            }
            .to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "double-escape-action".to_string(),
            label: "Double-escape action".to_string(),
            description: "Action when pressing Escape twice with empty editor".to_string(),
            current_value: double_escape_action.to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "tree-filter-mode".to_string(),
            label: "Tree filter mode".to_string(),
            description: "Default filter when opening /tree".to_string(),
            current_value: tree_filter_mode.to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "warnings".to_string(),
            label: "Warnings".to_string(),
            description: "Enable or disable individual warnings".to_string(),
            current_value: "configure".to_string(),
            has_submenu: true,
        },
        SettingDisplayItem {
            id: "thinking".to_string(),
            label: "Thinking level".to_string(),
            description: "Reasoning depth for thinking-capable models".to_string(),
            current_value: thinking_level.to_string(),
            has_submenu: true,
        },
        SettingDisplayItem {
            id: "theme".to_string(),
            label: "Theme".to_string(),
            description: "Color theme for the interface".to_string(),
            current_value: current_theme.to_string(),
            has_submenu: true,
        },
        SettingDisplayItem {
            id: "show-hardware-cursor".to_string(),
            label: "Show hardware cursor".to_string(),
            description: "Show the terminal cursor while still positioning it for IME support"
                .to_string(),
            current_value: if show_hardware_cursor {
                "enabled"
            } else {
                "disabled"
            }
            .to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "editor-padding".to_string(),
            label: "Editor padding".to_string(),
            description: "Horizontal padding for input editor (0-3)".to_string(),
            current_value: editor_padding_x.to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "autocomplete-max-visible".to_string(),
            label: "Autocomplete max items".to_string(),
            description: "Max visible items in autocomplete dropdown (3-20)".to_string(),
            current_value: autocomplete_max_visible.to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "clear-on-shrink".to_string(),
            label: "Clear on shrink".to_string(),
            description: "Clear empty rows when content shrinks (may cause flicker)".to_string(),
            current_value: if clear_on_shrink {
                "enabled"
            } else {
                "disabled"
            }
            .to_string(),
            has_submenu: false,
        },
        SettingDisplayItem {
            id: "terminal-progress".to_string(),
            label: "Terminal progress".to_string(),
            description: "Show OSC 9;4 progress indicators in the terminal tab bar".to_string(),
            current_value: if show_terminal_progress {
                "enabled"
            } else {
                "disabled"
            }
            .to_string(),
            has_submenu: false,
        },
    ]
}

fn format_http_idle_timeout(ms: u64) -> String {
    if ms == 0 {
        return "disabled".to_string();
    }
    if ms >= 60000 {
        format!("{}m", ms / 60000)
    } else if ms >= 1000 {
        format!("{}s", ms / 1000)
    } else {
        format!("{}ms", ms)
    }
}

/// Render settings selector
pub fn render_settings_selector(
    items: &[SettingDisplayItem],
    selected_index: usize,
    search_query: &str,
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));
    lines.push(ToolTheme::fg("accent", &border));
    lines.push(String::new());

    // Search
    let search_display = if search_query.is_empty() {
        ToolTheme::fg("muted", "  Type to search...")
    } else {
        format!("  {}", search_query)
    };
    lines.push(search_display);
    lines.push(String::new());

    let max_visible = 10;
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

    for i in start..end {
        if let Some(item) = items.get(i) {
            let is_selected = i == selected_index;
            let has_submenu = item.has_submenu;

            let cursor = if is_selected {
                ToolTheme::fg("accent", "› ")
            } else {
                "  ".to_string()
            };

            let value_display = if has_submenu {
                format!(" {}>", ToolTheme::fg("accent", "▶"))
            } else {
                format!(
                    " {}",
                    ToolTheme::fg("dim", &format!("[{}]", item.current_value))
                )
            };

            let label = if is_selected {
                ToolTheme::bold(&item.label)
            } else {
                item.label.clone()
            };
            let desc = ToolTheme::fg("muted", &format!(" {}", item.description));

            let line = format!("{}{}{}{}", cursor, label, value_display, desc);
            lines.push(line);
        }
    }

    if total > max_visible {
        lines.push(ToolTheme::fg(
            "muted",
            &format!("  ({}/{})", selected_index + 1, total),
        ));
    }

    lines.push(String::new());
    lines.push(ToolTheme::fg(
        "dim",
        "  ↑↓: navigate · Enter: toggle · Esc: close",
    ));
    lines.push(String::new());
    lines.push(ToolTheme::fg("accent", &border));
    lines
}

/// Render a submenu selector (for thinking level, theme, etc.)
pub fn render_submenu_selector(
    title: &str,
    description: &str,
    options: &[(&str, &str)],
    selected_index: usize,
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));
    lines.push(ToolTheme::fg("accent", &border));
    lines.push(ToolTheme::fg(
        "accent",
        &format!("\x1b[1m{}\x1b[22m", title),
    ));
    if !description.is_empty() {
        lines.push(ToolTheme::fg("muted", description));
    }
    lines.push(String::new());

    let max_visible = 10;
    let total = options.len();
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
        if let Some((value, desc)) = options.get(i) {
            let is_selected = i == selected_index;
            let cursor = if is_selected {
                ToolTheme::fg("accent", "→ ")
            } else {
                "  ".to_string()
            };
            let line = if is_selected {
                format!("{}{}", cursor, ToolTheme::fg("accent", value))
            } else {
                format!("{}  {}", cursor, value)
            };
            lines.push(format!("{}  {}", line, ToolTheme::fg("muted", desc)));
        }
    }

    if total > max_visible {
        lines.push(ToolTheme::fg(
            "muted",
            &format!("  ({}/{})", selected_index + 1, total),
        ));
    }

    lines.push(String::new());
    lines.push(ToolTheme::fg("dim", "  Enter: select · Esc: go back"));
    lines.push(ToolTheme::fg("accent", &border));
    lines
}

/// Render warning settings submenu
pub fn render_warning_settings(
    anthropic_extra_usage: bool,
    selected_index: usize,
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));
    lines.push(ToolTheme::fg("accent", &border));
    lines.push(ToolTheme::fg(
        "accent",
        &format!("\x1b[1m{}\x1b[22m", "Warning Settings"),
    ));
    lines.push(String::new());

    let items = [(
        "anthropic-extra-usage",
        "Anthropic extra usage",
        "Warn when Anthropic subscription auth may use paid extra usage",
    )];

    for (i, (id, label, desc)) in items.iter().enumerate() {
        let is_selected = i == selected_index;
        let cursor = if is_selected { "› " } else { "  " };
        let value = if id == &"anthropic-extra-usage" {
            if anthropic_extra_usage {
                "enabled"
            } else {
                "disabled"
            }
        } else {
            ""
        };
        let line = format!(
            "{}{} {}  {}",
            cursor,
            label,
            ToolTheme::fg("muted", &format!("[{}]", value)),
            ToolTheme::fg("muted", desc)
        );
        lines.push(line);
    }

    lines.push(String::new());
    lines.push(ToolTheme::fg("dim", "  Enter: toggle · Esc: go back"));
    lines.push(ToolTheme::fg("accent", &border));
    lines
}
