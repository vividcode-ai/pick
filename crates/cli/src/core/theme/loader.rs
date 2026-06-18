use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::types::*;

const EMBEDDED_DARK_THEME: &str = include_str!("../../../themes/dark.json");
const EMBEDDED_LIGHT_THEME: &str = include_str!("../../../themes/light.json");

static BUILTIN_THEMES_CACHE: std::sync::OnceLock<HashMap<String, ThemeJson>> =
    std::sync::OnceLock::new();

fn get_builtin_themes() -> &'static HashMap<String, ThemeJson> {
    BUILTIN_THEMES_CACHE.get_or_init(|| {
        let mut themes = HashMap::new();

        let themes_dir = crate::config::get_themes_dir();
        let dark_path = themes_dir.join("dark.json");
        let light_path = themes_dir.join("light.json");

        let dark_loaded = if let Ok(content) = std::fs::read_to_string(&dark_path) {
            serde_json::from_str::<ThemeJson>(&content).ok()
        } else {
            None
        };

        let light_loaded = if let Ok(content) = std::fs::read_to_string(&light_path) {
            serde_json::from_str::<ThemeJson>(&content).ok()
        } else {
            None
        };

        let dark_theme = dark_loaded.unwrap_or_else(|| {
            serde_json::from_str(EMBEDDED_DARK_THEME)
                .expect("Embedded dark.json must be valid JSON")
        });

        let light_theme = light_loaded.unwrap_or_else(|| {
            serde_json::from_str(EMBEDDED_LIGHT_THEME)
                .expect("Embedded light.json must be valid JSON")
        });

        themes.insert("dark".to_string(), dark_theme);
        themes.insert("light".to_string(), light_theme);

        themes
    })
}

pub fn get_available_themes() -> Vec<String> {
    get_available_themes_with_paths()
        .into_iter()
        .map(|t| t.name)
        .collect()
}

pub struct ThemeInfo {
    pub name: String,
    pub path: Option<String>,
}

pub fn get_available_themes_with_paths() -> Vec<ThemeInfo> {
    let themes_dir = crate::config::get_themes_dir();
    let mut result = Vec::new();
    let mut seen = HashSet::new();

    let mut add_theme = |name: String, path: Option<String>| {
        if seen.contains(&name) {
            return;
        }
        seen.insert(name.clone());
        result.push(ThemeInfo { name, path });
    };

    for name in get_builtin_themes().keys() {
        add_theme(name.clone(), Some(themes_dir.join(format!("{}.json", name)).to_string_lossy().to_string()));
    }

    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

pub fn load_json_themes_from_dir(dir: &Path) -> Vec<(String, ThemeJson)> {
    let mut themes = Vec::new();
    if !dir.exists() {
        return themes;
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(json) = serde_json::from_str::<ThemeJson>(&content) {
                        let name = json.name.clone();
                        themes.push((name, json));
                    }
                }
            }
        }
    }

    themes
}

fn load_theme_json(name: &str) -> Result<ThemeJson, String> {
    let builtin = get_builtin_themes();
    if let Some(json) = builtin.get(name) {
        return Ok(json.clone());
    }

    let custom_dir = crate::config::get_custom_themes_dir(&std::env::current_dir().unwrap());
    let theme_path = custom_dir.join(format!("{}.json", name));
    if theme_path.exists() {
        let content = std::fs::read_to_string(&theme_path)
            .map_err(|e| format!("Failed to read theme: {}", e))?;
        return serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse theme: {}", e));
    }

    Err(format!("Theme not found: {}", name))
}

pub fn create_theme_from_json(theme_json: &ThemeJson, mode: Option<ColorMode>) -> Theme {
    let color_mode = mode.unwrap_or(ColorMode::TrueColor);
    Theme::new(
        Some(theme_json.name.clone()),
        None,
        theme_json,
        color_mode,
    )
}

pub fn load_theme_from_path(theme_path: &str, mode: Option<ColorMode>) -> Result<Theme, String> {
    let content = std::fs::read_to_string(theme_path)
        .map_err(|e| format!("Failed to read theme file: {}", e))?;
    let theme_json: ThemeJson = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse theme JSON: {}", e))?;
    let color_mode = mode.unwrap_or(ColorMode::TrueColor);
    Ok(Theme::new(
        Some(theme_json.name.clone()),
        Some(theme_path.to_string()),
        &theme_json,
        color_mode,
    ))
}

fn load_theme(name: &str, mode: Option<ColorMode>) -> Result<Theme, String> {
    let theme_json = load_theme_json(name)?;
    let color_mode = mode.unwrap_or(ColorMode::TrueColor);
    Ok(Theme::new(
        Some(theme_json.name.clone()),
        None,
        &theme_json,
        color_mode,
    ))
}

pub fn get_theme_by_name(name: &str) -> Option<Theme> {
    load_theme(name, None).ok()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TerminalTheme {
    Dark,
    Light,
}

#[derive(Debug, Clone)]
pub struct TerminalThemeDetection {
    pub theme: TerminalTheme,
    pub source: String,
    pub detail: String,
    pub confidence: String,
}

pub fn get_theme_for_rgb(r: u8, g: u8, b: u8) -> TerminalTheme {
    if get_rgb_luminance(r, g, b) >= 0.5 {
        TerminalTheme::Light
    } else {
        TerminalTheme::Dark
    }
}

pub fn detect_terminal_background() -> TerminalThemeDetection {
    if let Ok(colorfgbg) = std::env::var("COLORFGBG") {
        let parts: Vec<&str> = colorfgbg.split(';').collect();
        if let Some(last) = parts.last() {
            if let Ok(bg) = last.trim().parse::<u8>() {
                let luminance = ansi256_luminance(bg);
                return TerminalThemeDetection {
                    theme: if luminance >= 0.5 { TerminalTheme::Light } else { TerminalTheme::Dark },
                    source: "COLORFGBG".to_string(),
                    detail: format!("background color index {}", bg),
                    confidence: "high".to_string(),
                };
            }
        }
    }

    TerminalThemeDetection {
        theme: TerminalTheme::Dark,
        source: "fallback".to_string(),
        detail: "no terminal background hint found".to_string(),
        confidence: "low".to_string(),
    }
}

pub fn get_default_theme() -> String {
    match detect_terminal_background().theme {
        TerminalTheme::Light => "light".to_string(),
        TerminalTheme::Dark => "dark".to_string(),
    }
}

fn ansi256_luminance(index: u8) -> f64 {
    let hex = ansi256_to_hex(index);
    if let Some(rgb) = hex_to_rgb(&hex) {
        get_rgb_luminance(rgb.r, rgb.g, rgb.b)
    } else {
        0.0
    }
}

use std::sync::OnceLock;

static GLOBAL_THEME: OnceLock<Arc<Mutex<Theme>>> = OnceLock::new();
static THEME_INITIALIZED: AtomicBool = AtomicBool::new(false);
static CURRENT_THEME_NAME: OnceLock<Mutex<String>> =
    OnceLock::new();
static THEME_WATCHER: OnceLock<Mutex<Option<crate::utils::fs_watch::FsWatcher>>> =
    OnceLock::new();
static ON_THEME_CHANGE: OnceLock<Mutex<Option<Box<dyn Fn() + Send>>>> =
    OnceLock::new();

fn get_current_theme_name_lock() -> &'static Mutex<String> {
    CURRENT_THEME_NAME.get_or_init(|| Mutex::new("dark".to_string()))
}

fn get_theme_watcher_lock() -> &'static Mutex<Option<crate::utils::fs_watch::FsWatcher>> {
    THEME_WATCHER.get_or_init(|| Mutex::new(None))
}

fn get_on_theme_change_lock() -> &'static Mutex<Option<Box<dyn Fn() + Send>>> {
    ON_THEME_CHANGE.get_or_init(|| Mutex::new(None))
}

pub fn on_theme_change(callback: Box<dyn Fn() + Send>) {
    if let Ok(mut guard) = get_on_theme_change_lock().lock() {
        *guard = Some(callback);
    }
}

fn start_theme_watcher() {
    stop_theme_watcher();

    let current_name = get_current_theme_name_lock().lock()
        .map(|n| n.clone())
        .unwrap_or_else(|_| "dark".to_string());

    if current_name == "dark" || current_name == "light" {
        return;
    }

    let custom_themes_dir = crate::config::get_custom_themes_dir(&std::env::current_dir().unwrap());
    let watched_file_name = format!("{}.json", current_name);
    let theme_file = custom_themes_dir.join(&watched_file_name);

    if !theme_file.exists() {
        return;
    }

    let watched_name = current_name.clone();

    let callback = move |_path: String| {
        let current = get_current_theme_name_lock().lock()
            .map(|n| n.clone())
            .unwrap_or_default();
        if current != watched_name {
            return;
        }

        let theme_path = std::path::PathBuf::from(&_path);
        if !theme_path.exists() {
            return;
        }

        let theme_path_str = theme_path.to_string_lossy().to_string();
        match load_theme_from_path(&theme_path_str, None) {
            Ok(reloaded) => {
                if let Some(global) = GLOBAL_THEME.get() {
                    if let Ok(mut t) = global.lock() {
                        *t = reloaded;
                    }
                }
                if let Ok(guard) = get_on_theme_change_lock().lock() {
                    if let Some(ref cb) = *guard {
                        cb();
                    }
                }
            }
            Err(_) => {}
        }
    };

    let on_error = Box::new(|| {
        tracing::warn!("Theme watcher error, stopping");
        stop_theme_watcher();
    });

    if let Some(watcher) = crate::utils::fs_watch::watch_with_error_handler(
        &theme_file,
        callback,
        on_error,
    ) {
        if let Ok(mut guard) = get_theme_watcher_lock().lock() {
            *guard = Some(watcher);
        }
    }
}

fn stop_theme_watcher() {
    if let Ok(mut guard) = get_theme_watcher_lock().lock() {
        if let Some(watcher) = guard.take() {
            let _ = watcher.shutdown.send(());
        }
    }
}

pub fn global_theme() -> Arc<Mutex<Theme>> {
    GLOBAL_THEME.get_or_init(|| -> Arc<Mutex<Theme>> {
        let name = get_default_theme();
        let theme = load_theme(&name, None).unwrap_or_else(|_| {
            let dark_json = get_builtin_themes().get("dark").cloned()
                .unwrap_or_else(|| {
                    serde_json::from_str(r#"{"name":"dark","colors":{}}"#).unwrap()
                });
            Theme::new(Some("dark".to_string()), None, &dark_json, ColorMode::TrueColor)
        });
        Arc::new(Mutex::new(theme))
    }).clone()
}

pub fn init_theme(theme_name: Option<&str>, enable_watcher: bool) {
    stop_theme_watcher();

    let name = theme_name.map(|s| s.to_string()).unwrap_or_else(get_default_theme);
    if let Ok(mut current) = get_current_theme_name_lock().lock() {
        *current = name.clone();
    }

    let theme = load_theme(&name, None).unwrap_or_else(|_| {
        let dark_json = get_builtin_themes().get("dark").cloned()
            .unwrap_or_else(|| {
                serde_json::from_str(r#"{"name":"dark","colors":{}}"#).unwrap()
            });
        Theme::new(Some("dark".to_string()), None, &dark_json, ColorMode::TrueColor)
    });

    let _ = GLOBAL_THEME.set(Arc::new(Mutex::new(theme)));
    THEME_INITIALIZED.store(true, Ordering::Release);

    if enable_watcher {
        start_theme_watcher();
    }
}

pub fn set_theme(name: &str, enable_watcher: bool) -> Result<(), String> {
    let theme = load_theme(name, None)?;
    if let Ok(mut current) = get_current_theme_name_lock().lock() {
        *current = name.to_string();
    }
    if let Some(global) = GLOBAL_THEME.get() {
        if let Ok(mut t) = global.lock() {
            *t = theme;
        }
    }

    if enable_watcher {
        start_theme_watcher();
    } else {
        stop_theme_watcher();
    }

    Ok(())
}

pub fn get_resolved_theme_colors(theme_name: Option<&str>) -> HashMap<String, String> {
    let name = theme_name.map(|s| s.to_string())
        .unwrap_or_else(|| {
            get_current_theme_name_lock().lock().map(|n| n.clone()).unwrap_or_else(|_| "dark".to_string())
        });
    let is_light = name == "light";
    let default_text = if is_light { "#000000" } else { "#e5e5e7" };

    let theme_json = load_theme_json(&name).unwrap_or_else(|_| {
        get_builtin_themes().get("dark").cloned()
            .unwrap_or_else(|| {
                serde_json::from_str(r#"{"name":"dark","colors":{}}"#).unwrap()
            })
    });

    let resolved = resolve_theme_colors(&theme_json);
    let mut css_colors = HashMap::new();

    for (key, value) in resolved {
        let css = match &value {
            ResolvedColor::Index(idx) => ansi256_to_hex(*idx),
            ResolvedColor::Hex(h) => h.clone(),
            ResolvedColor::Empty => default_text.to_string(),
        };
        css_colors.insert(key, css);
    }

    css_colors
}

fn resolve_theme_colors(theme_json: &ThemeJson) -> HashMap<String, ResolvedColor> {
    let mut result = HashMap::new();
    let vars = &theme_json.vars;

    let pairs = vec![
        ("accent", &theme_json.colors.accent),
        ("border", &theme_json.colors.border),
        ("borderAccent", &theme_json.colors.border_accent),
        ("borderMuted", &theme_json.colors.border_muted),
        ("success", &theme_json.colors.success),
        ("error", &theme_json.colors.error),
        ("warning", &theme_json.colors.warning),
        ("muted", &theme_json.colors.muted),
        ("dim", &theme_json.colors.dim),
        ("text", &theme_json.colors.text),
        ("thinkingText", &theme_json.colors.thinking_text),
        ("selectedBg", &theme_json.colors.selected_bg),
        ("userMessageBg", &theme_json.colors.user_message_bg),
        ("userMessageText", &theme_json.colors.user_message_text),
        ("customMessageBg", &theme_json.colors.custom_message_bg),
        ("customMessageText", &theme_json.colors.custom_message_text),
        ("customMessageLabel", &theme_json.colors.custom_message_label),
        ("toolPendingBg", &theme_json.colors.tool_pending_bg),
        ("toolSuccessBg", &theme_json.colors.tool_success_bg),
        ("toolErrorBg", &theme_json.colors.tool_error_bg),
        ("toolTitle", &theme_json.colors.tool_title),
        ("toolOutput", &theme_json.colors.tool_output),
        ("mdHeading", &theme_json.colors.md_heading),
        ("mdLink", &theme_json.colors.md_link),
        ("mdLinkUrl", &theme_json.colors.md_link_url),
        ("mdCode", &theme_json.colors.md_code),
        ("mdCodeBlock", &theme_json.colors.md_code_block),
        ("mdCodeBlockBorder", &theme_json.colors.md_code_block_border),
        ("mdQuote", &theme_json.colors.md_quote),
        ("mdQuoteBorder", &theme_json.colors.md_quote_border),
        ("mdHr", &theme_json.colors.md_hr),
        ("mdListBullet", &theme_json.colors.md_list_bullet),
        ("toolDiffAdded", &theme_json.colors.tool_diff_added),
        ("toolDiffRemoved", &theme_json.colors.tool_diff_removed),
        ("toolDiffContext", &theme_json.colors.tool_diff_context),
        ("syntaxComment", &theme_json.colors.syntax_comment),
        ("syntaxKeyword", &theme_json.colors.syntax_keyword),
        ("syntaxFunction", &theme_json.colors.syntax_function),
        ("syntaxVariable", &theme_json.colors.syntax_variable),
        ("syntaxString", &theme_json.colors.syntax_string),
        ("syntaxNumber", &theme_json.colors.syntax_number),
        ("syntaxType", &theme_json.colors.syntax_type),
        ("syntaxOperator", &theme_json.colors.syntax_operator),
        ("syntaxPunctuation", &theme_json.colors.syntax_punctuation),
        ("thinkingOff", &theme_json.colors.thinking_off),
        ("thinkingMinimal", &theme_json.colors.thinking_minimal),
        ("thinkingLow", &theme_json.colors.thinking_low),
        ("thinkingMedium", &theme_json.colors.thinking_medium),
        ("thinkingHigh", &theme_json.colors.thinking_high),
        ("thinkingXhigh", &theme_json.colors.thinking_xhigh),
        ("bashMode", &theme_json.colors.bash_mode),
    ];

    for (key, value) in pairs {
        result.insert(key.to_string(), color_value_to_resolved(value, vars));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_to_rgb() {
        let rgb = hex_to_rgb("#ff0000").unwrap();
        assert_eq!(rgb.r, 255);
        assert_eq!(rgb.g, 0);
        assert_eq!(rgb.b, 0);
    }

    #[test]
    fn test_rgb_luminance() {
        let lum = get_rgb_luminance(255, 255, 255);
        assert!(lum > 0.9);
        let lum = get_rgb_luminance(0, 0, 0);
        assert!(lum < 0.01);
    }

    #[test]
    fn test_theme_for_rgb_light() {
        assert_eq!(get_theme_for_rgb(255, 255, 255), TerminalTheme::Light);
        assert_eq!(get_theme_for_rgb(0, 0, 0), TerminalTheme::Dark);
    }

    #[test]
    fn test_get_language_from_path() {
        assert_eq!(get_language_from_path("main.rs"), Some("rust"));
        assert_eq!(get_language_from_path("main.ts"), Some("typescript"));
        assert_eq!(get_language_from_path("main.py"), Some("python"));
        assert_eq!(get_language_from_path("unknown.xyz"), None);
    }

    #[test]
    fn test_ansi256_to_hex_basic() {
        assert_eq!(ansi256_to_hex(0), "#000000");
        assert_eq!(ansi256_to_hex(15), "#ffffff");
    }

    #[test]
    fn test_hex_to_256() {
        let idx = hex_to_256("#ff0000").unwrap();
        assert!(idx >= 16 && idx <= 231);
    }
}
