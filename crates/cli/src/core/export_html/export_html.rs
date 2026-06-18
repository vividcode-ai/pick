//! HTML session export


use std::collections::HashMap;
use std::path::Path;

/// Theme colors for HTML export
pub struct ExportThemeColors {
    pub page_bg: String,
    pub card_bg: String,
    pub info_bg: String,
    pub theme_vars: HashMap<String, String>,
}

/// Options for HTML export
pub struct ExportOptions {
    pub output_path: Option<String>,
    pub theme_name: Option<String>,
}

/// A rendered HTML tool call
pub struct RenderedToolHtml {
    pub call_html: Option<String>,
    pub result_html_collapsed: Option<String>,
    pub result_html_expanded: Option<String>,
}

/// Rendered session data for HTML template
pub struct SessionData {
    pub header: serde_json::Value,
    pub entries: Vec<serde_json::Value>,
    pub leaf_id: Option<String>,
    pub system_prompt: Option<String>,
    pub tools: Option<Vec<ToolDef>>,
    pub rendered_tools: Option<HashMap<String, RenderedToolHtml>>,
}

/// Tool definition for HTML export
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: Option<serde_json::Value>,
}

/// Calculate relative luminance (0-1, higher = lighter)
fn get_luminance(r: u8, g: u8, b: u8) -> f64 {
    let to_linear = |c: u8| {
        let s = c as f64 / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * to_linear(r) + 0.7152 * to_linear(g) + 0.0722 * to_linear(b)
}

/// Parse a hex color string (#RRGGBB) to RGB
fn parse_hex_color(color: &str) -> Option<(u8, u8, u8)> {
    let hex = color.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some((r, g, b))
    } else {
        None
    }
}

/// Parse a color string to RGB (supports hex #RRGGBB and rgb(r,g,b))
pub fn parse_color(color: &str) -> Option<(u8, u8, u8)> {
    if color.starts_with('#') {
        return parse_hex_color(color);
    }
    if color.starts_with("rgb(") {
        let inner = color.trim_start_matches("rgb(").trim_end_matches(')');
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        if parts.len() == 3 {
            let r = parts[0].parse::<u8>().ok()?;
            let g = parts[1].parse::<u8>().ok()?;
            let b = parts[2].parse::<u8>().ok()?;
            return Some((r, g, b));
        }
    }
    None
}

/// Adjust color brightness. Factor > 1 lightens, < 1 darkens.
fn adjust_brightness(color: &str, factor: f64) -> String {
    if let Some((r, g, b)) = parse_color(color) {
        let adjust = |c: u8| (c as f64 * factor).round().clamp(0.0, 255.0) as u8;
        format!("rgb({}, {}, {})", adjust(r), adjust(g), adjust(b))
    } else {
        color.to_string()
    }
}

/// Derive export background colors from a base color
pub fn derive_export_colors(base_color: &str) -> ExportColors {
    let default = ExportColors {
        page_bg: "rgb(24, 24, 30)".to_string(),
        card_bg: "rgb(30, 30, 36)".to_string(),
        info_bg: "rgb(60, 55, 40)".to_string(),
    };

    let (r, g, b) = match parse_color(base_color) {
        Some(c) => c,
        None => return default,
    };

    let luminance = get_luminance(r, g, b);
    let is_light = luminance > 0.5;

    if is_light {
        ExportColors {
            page_bg: adjust_brightness(base_color, 0.96),
            card_bg: base_color.to_string(),
            info_bg: format!("rgb({}, {}, {})",
                (r as u16).min(255),
                (g as u16).min(255),
                (b as u16).saturating_sub(20)),
        }
    } else {
        ExportColors {
            page_bg: adjust_brightness(base_color, 0.7),
            card_bg: adjust_brightness(base_color, 0.85),
            info_bg: format!("rgb({}, {}, {})",
                (r as u16 + 20).min(255),
                (g as u16 + 15).min(255),
                b),
        }
    }
}

/// Derived export color set
pub struct ExportColors {
    pub page_bg: String,
    pub card_bg: String,
    pub info_bg: String,
}

impl Default for ExportColors {
    fn default() -> Self {
        Self {
            page_bg: "rgb(24, 24, 30)".to_string(),
            card_bg: "rgb(30, 30, 36)".to_string(),
            info_bg: "rgb(60, 55, 40)".to_string(),
        }
    }
}

/// Generate CSS custom property declarations from theme colors
pub fn generate_theme_vars(theme_colors: &HashMap<String, String>, export_colors: &ExportColors) -> String {
    let mut lines: Vec<String> = Vec::new();
    for (key, value) in theme_colors {
        lines.push(format!("--{}: {};", key, value));
    }
    lines.push(format!("--exportPageBg: {};", export_colors.page_bg));
    lines.push(format!("--exportCardBg: {};", export_colors.card_bg));
    lines.push(format!("--exportInfoBg: {};", export_colors.info_bg));
    lines.join("\n      ")
}

/// CSS template string for export
fn get_template_css() -> &'static str {
    include_str!("templates/template.css")
}

/// JS template string for export
fn get_template_js() -> &'static str {
    include_str!("templates/template.js")
}

/// Generate the full HTML document for session export
pub fn generate_html(session_data: &SessionData, theme_colors: &HashMap<String, String>, export_colors: &ExportColors) -> String {
    let theme_vars = generate_theme_vars(theme_colors, export_colors);
    let body_bg = &export_colors.page_bg;
    let container_bg = &export_colors.card_bg;
    let info_bg = &export_colors.info_bg;

    // Build session data JSON (manual construction to avoid macro issues with closures)
    let mut session_obj = serde_json::Map::new();
    session_obj.insert("header".to_string(), session_data.header.clone());
    session_obj.insert("entries".to_string(), serde_json::Value::Array(session_data.entries.clone()));
    session_obj.insert("leafId".to_string(), session_data.leaf_id.clone().map_or(serde_json::Value::Null, serde_json::Value::String));
    session_obj.insert("systemPrompt".to_string(), session_data.system_prompt.clone().map_or(serde_json::Value::Null, serde_json::Value::String));

    // Tools array
    let tools_val = session_data.tools.as_ref().map(|tools| {
        serde_json::Value::Array(tools.iter().map(|t| {
            let mut obj = serde_json::Map::new();
            obj.insert("name".to_string(), serde_json::Value::String(t.name.clone()));
            obj.insert("description".to_string(), serde_json::Value::String(t.description.clone()));
            obj.insert("parameters".to_string(), t.parameters.clone().unwrap_or(serde_json::Value::Null));
            serde_json::Value::Object(obj)
        }).collect())
    }).unwrap_or(serde_json::Value::Null);
    session_obj.insert("tools".to_string(), tools_val);

    // Rendered tools
    let rendered_val = session_data.rendered_tools.as_ref().map(|rt| {
        let mut map = serde_json::Map::new();
        for (id, html) in rt {
            let mut obj = serde_json::Map::new();
            if let Some(ref call) = html.call_html {
                obj.insert("callHtml".to_string(), serde_json::Value::String(call.clone()));
            }
            if let Some(ref collapsed) = html.result_html_collapsed {
                obj.insert("resultHtmlCollapsed".to_string(), serde_json::Value::String(collapsed.clone()));
            }
            if let Some(ref expanded) = html.result_html_expanded {
                obj.insert("resultHtmlExpanded".to_string(), serde_json::Value::String(expanded.clone()));
            }
            map.insert(id.clone(), serde_json::Value::Object(obj));
        }
        serde_json::Value::Object(map)
    }).unwrap_or(serde_json::Value::Null);
    session_obj.insert("renderedTools".to_string(), rendered_val);

    let session_json = serde_json::to_string(&serde_json::Value::Object(session_obj)).unwrap_or_default();

    let session_data_base64 = base64_encode(&session_json);

    // Build the CSS with theme variables injected
    let css_raw = get_template_css();
    let css = css_raw
        .replace("{{THEME_VARS}}", &theme_vars)
        .replace("{{BODY_BG}}", body_bg)
        .replace("{{CONTAINER_BG}}", container_bg)
        .replace("{{INFO_BG}}", info_bg);

    let js = get_template_js();

    // Load vendor libs
    let marked_js = include_str!("templates/vendor/marked.min.js");
    let highlight_js = include_str!("templates/vendor/highlight.min.js");

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Session Export</title>
  <style>
{}
  </style>
</head>
<body>
  <button id="hamburger" title="Open sidebar"><svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" stroke="none"><circle cx="6" cy="6" r="2.5"/><circle cx="6" cy="18" r="2.5"/><circle cx="18" cy="12" r="2.5"/><rect x="5" y="6" width="2" height="12"/><path d="M6 12h10c1 0 2 0 2-2V8"/></svg></button>
  <div id="sidebar-overlay"></div>
  <div id="app">
    <aside id="sidebar">
      <div class="sidebar-header">
        <div class="sidebar-controls">
          <input type="text" class="sidebar-search" id="tree-search" placeholder="Search...">
        </div>
        <div class="sidebar-filters">
          <button class="filter-btn active" data-filter="default" title="Hide settings entries">Default</button>
          <button class="filter-btn" data-filter="no-tools" title="Default minus tool results">No-tools</button>
          <button class="filter-btn" data-filter="user-only" title="Only user messages">User</button>
          <button class="filter-btn" data-filter="labeled-only" title="Only labeled entries">Labeled</button>
          <button class="filter-btn" data-filter="all" title="Show everything">All</button>
          <button class="sidebar-close" id="sidebar-close" title="Close">✕</button>
        </div>
      </div>
      <div class="tree-container" id="tree-container"></div>
      <div class="tree-status" id="tree-status"></div>
    </aside>
    <div id="sidebar-resizer" role="separator" aria-orientation="vertical" aria-label="Resize session tree sidebar"></div>
    <main id="content">
      <div id="header-container"></div>
      <div id="messages"></div>
    </main>
    <div id="image-modal" class="image-modal">
      <img id="modal-image" src="" alt="">
    </div>
  </div>

  <script id="session-data" type="application/json">{}</script>

  <!-- Vendored libraries -->
  <script>{}</script>

  <!-- highlight.js -->
  <script>{}</script>

  <!-- Main application code -->
  <script>
{}
  </script>
</body>
</html>"#,
        css, session_data_base64, marked_js, highlight_js, js
    )
}

/// Simple base64 encoding (avoids external dependency for this one function)
fn base64_encode(data: &str) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = data.as_bytes();
    let mut result = String::new();

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}

/// Export session to HTML (standalone, without session manager)
pub fn export_session_to_html(
    session_data: &SessionData,
    theme_colors: &HashMap<String, String>,
    export_colors: &ExportColors,
    output_path: Option<&Path>,
) -> Result<String, String> {
    let html = generate_html(session_data, theme_colors, export_colors);

    if let Some(path) = output_path {
        std::fs::write(path, &html)
            .map_err(|e| format!("Failed to write export file: {}", e))?;
        Ok(path.to_string_lossy().to_string())
    } else {
        // Return HTML as string if no output path given
        Ok(html)
    }
}

/// Export a session from raw session data
pub fn export_from_data(
    entries: Vec<serde_json::Value>,
    header: serde_json::Value,
    leaf_id: Option<String>,
    system_prompt: Option<String>,
    tools: Option<Vec<ToolDef>>,
    options: Option<&ExportOptions>,
) -> Result<String, String> {
    let theme_colors = HashMap::new(); // Would use theme system
    let export_colors = ExportColors::default();

    let session_data = SessionData {
        header,
        entries,
        leaf_id,
        system_prompt,
        tools,
        rendered_tools: None,
    };

    let html = generate_html(&session_data, &theme_colors, &export_colors);

    if let Some(opts) = options {
        if let Some(ref path) = opts.output_path {
            std::fs::write(path, &html)
                .map_err(|e| format!("Failed to write export file: {}", e))?;
            return Ok(path.clone());
        }
    }

    Ok(html)
}
