//! Keybinding hint formatting utilities


use crate::core::tools::render_utils::ToolTheme;

/// Format a key string for display (e.g. "ctrl+c" → "Ctrl+C")
pub fn format_key_text(key: &str) -> String {
    key.split('/')
        .map(|k| {
            k.split('+')
                .map(|part| {
                    let lower = part.to_lowercase();
                    match lower.as_str() {
                        "ctrl" => "Ctrl".to_string(),
                        "alt" => "Alt".to_string(),
                        "shift" => "Shift".to_string(),
                        "meta" => "Meta".to_string(),
                        "enter" => "Enter".to_string(),
                        "space" => "Space".to_string(),
                        "tab" => "Tab".to_string(),
                        "escape" | "esc" => "Esc".to_string(),
                        "backspace" => "Backspace".to_string(),
                        "delete" => "Delete".to_string(),
                        "up" => "↑".to_string(),
                        "down" => "↓".to_string(),
                        "left" => "←".to_string(),
                        "right" => "→".to_string(),
                        "pageup" => "PgUp".to_string(),
                        "pagedown" => "PgDn".to_string(),
                        "home" => "Home".to_string(),
                        "end" => "End".to_string(),
                        _ => {
                            if lower.len() == 1 {
                                lower.to_uppercase()
                            } else {
                                let mut chars = lower.chars();
                                match chars.next() {
                                    Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                                    None => String::new(),
                                }
                            }
                        }
                    }
                })
                .collect::<Vec<_>>()
                .join("+")
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Create a keybinding hint line (dim key + muted description)
pub fn key_hint(key: &str, description: &str) -> String {
    format!(
        "{}{}",
        ToolTheme::fg("dim", &format_key_text(key)),
        ToolTheme::fg("muted", &format!(" {}", description))
    )
}
