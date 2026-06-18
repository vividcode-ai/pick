//! ANSI escape code to HTML converter


/// Standard ANSI color palette (0-15)
const ANSI_COLORS: [&str; 16] = [
    "#000000", // 0: black
    "#800000", // 1: red
    "#008000", // 2: green
    "#808000", // 3: yellow
    "#000080", // 4: blue
    "#800080", // 5: magenta
    "#008080", // 6: cyan
    "#c0c0c0", // 7: white
    "#808080", // 8: bright black
    "#ff0000", // 9: bright red
    "#00ff00", // 10: bright green
    "#ffff00", // 11: bright yellow
    "#0000ff", // 12: bright blue
    "#ff00ff", // 13: bright magenta
    "#00ffff", // 14: bright cyan
    "#ffffff", // 15: bright white
];

/// Convert 256-color index to hex color string
fn color256_to_hex(index: u8) -> String {
    let idx = index as usize;
    // Standard colors (0-15)
    if idx < 16 {
        return ANSI_COLORS[idx].to_string();
    }

    // Color cube (16-231): 6x6x6 = 216 colors
    if idx < 232 {
        let cube_index = idx - 16;
        let r = cube_index / 36;
        let g = (cube_index % 36) / 6;
        let b = cube_index % 6;
        let to_component = |n: usize| if n == 0 { 0 } else { 55 + n * 40 };
        format!("#{:02x}{:02x}{:02x}", to_component(r), to_component(g), to_component(b))
    } else {
        // Grayscale (232-255): 24 shades
        let gray = 8 + (idx - 232) * 10;
        format!("#{:02x}{:02x}{:02x}", gray, gray, gray)
    }
}

/// Escape HTML special characters
fn escape_html(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&#039;".to_string(),
            _ => c.to_string(),
        })
        .collect()
}

#[derive(Debug, Clone, Default)]
struct TextStyle {
    fg: Option<String>,
    bg: Option<String>,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
}

fn style_to_inline_css(style: &TextStyle) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(ref fg) = style.fg {
        parts.push(format!("color:{}", fg));
    }
    if let Some(ref bg) = style.bg {
        parts.push(format!("background-color:{}", bg));
    }
    if style.bold {
        parts.push("font-weight:bold".to_string());
    }
    if style.dim {
        parts.push("opacity:0.6".to_string());
    }
    if style.italic {
        parts.push("font-style:italic".to_string());
    }
    if style.underline {
        parts.push("text-decoration:underline".to_string());
    }
    parts.join(";")
}

fn has_style(style: &TextStyle) -> bool {
    style.fg.is_some() || style.bg.is_some() || style.bold || style.dim || style.italic || style.underline
}

/// Parse ANSI SGR (Select Graphic Rendition) codes and update style
fn apply_sgr_code(params: &[u16], style: &mut TextStyle) {
    let mut i = 0;
    while i < params.len() {
        let code = params[i] as u16;

        match code {
            0 => {
                // Reset all
                style.fg = None;
                style.bg = None;
                style.bold = false;
                style.dim = false;
                style.italic = false;
                style.underline = false;
            }
            1 => style.bold = true,
            2 => style.dim = true,
            3 => style.italic = true,
            4 => style.underline = true,
            22 => {
                style.bold = false;
                style.dim = false;
            }
            23 => style.italic = false,
            24 => style.underline = false,
            30..=37 => {
                // Standard foreground
                style.fg = Some(ANSI_COLORS[(code - 30) as usize].to_string());
            }
            38 => {
                // Extended foreground
                if i + 2 < params.len() && params[i + 1] == 5 {
                    // 256-color: 38;5;N
                    style.fg = Some(color256_to_hex(params[i + 2] as u8));
                    i += 2;
                } else if i + 4 < params.len() && params[i + 1] == 2 {
                    // RGB: 38;2;R;G;B
                    let r = params[i + 2];
                    let g = params[i + 3];
                    let b = params[i + 4];
                    style.fg = Some(format!("rgb({},{},{})", r, g, b));
                    i += 4;
                }
            }
            39 => style.fg = None,   // Default foreground
            40..=47 => {
                // Standard background
                style.bg = Some(ANSI_COLORS[(code - 40) as usize].to_string());
            }
            48 => {
                // Extended background
                if i + 2 < params.len() && params[i + 1] == 5 {
                    // 256-color: 48;5;N
                    style.bg = Some(color256_to_hex(params[i + 2] as u8));
                    i += 2;
                } else if i + 4 < params.len() && params[i + 1] == 2 {
                    // RGB: 48;2;R;G;B
                    let r = params[i + 2];
                    let g = params[i + 3];
                    let b = params[i + 4];
                    style.bg = Some(format!("rgb({},{},{})", r, g, b));
                    i += 4;
                }
            }
            49 => style.bg = None,   // Default background
            90..=97 => {
                // Bright foreground
                style.fg = Some(ANSI_COLORS[(code - 90 + 8) as usize].to_string());
            }
            100..=107 => {
                // Bright background
                style.bg = Some(ANSI_COLORS[(code - 100 + 8) as usize].to_string());
            }
            _ => {}
        }
        i += 1;
    }
}

/// Match ANSI escape sequences: ESC[ followed by params and ending with 'm'
/// Regex: \x1b\[([\d;]*)m
fn parse_ansi_sequences(text: &str) -> Vec<(usize, usize, Vec<u16>)> {
    let mut sequences = Vec::new();
    let mut pos = 0;
    let bytes = text.as_bytes();

    while pos < bytes.len() {
        if bytes[pos] == 0x1b && pos + 1 < bytes.len() && bytes[pos + 1] == b'[' {
            let start = pos;
            pos += 2; // skip ESC[
            let param_start = pos;
            while pos < bytes.len() && bytes[pos] != b'm' {
                pos += 1;
            }
            if pos < bytes.len() && bytes[pos] == b'm' {
                let param_str = std::str::from_utf8(&bytes[param_start..pos]).unwrap_or("");
                let params: Vec<u16> = if param_str.is_empty() {
                    vec![0]
                } else {
                    param_str.split(';')
                        .filter_map(|s| s.parse::<u16>().ok())
                        .collect()
                };
                sequences.push((start, pos + 1, params));
                pos += 1;
            }
        } else {
            pos += 1;
        }
    }

    sequences
}

/// Convert ANSI-escaped text to HTML with inline styles
pub fn ansi_to_html(text: &str) -> String {
    let mut style = TextStyle::default();
    let mut result = String::new();
    let mut last_index = 0;
    let mut in_span = false;

    let sequences = parse_ansi_sequences(text);

    for (seq_start, seq_end, params) in &sequences {
        // Add text before this escape sequence
        let before_text = &text[last_index..*seq_start];
        if !before_text.is_empty() {
            result.push_str(&escape_html(before_text));
        }

        // Close existing span if we have one
        if in_span {
            result.push_str("</span>");
            in_span = false;
        }

        // Apply the codes
        apply_sgr_code(params, &mut style);

        // Open new span if we have any styling
        if has_style(&style) {
            result.push_str(&format!("<span style=\"{}\">", style_to_inline_css(&style)));
            in_span = true;
        }

        last_index = *seq_end;
    }

    // Add remaining text
    let remaining_text = &text[last_index..];
    if !remaining_text.is_empty() {
        result.push_str(&escape_html(remaining_text));
    }

    // Close any open span
    if in_span {
        result.push_str("</span>");
    }

    result
}

/// Convert array of ANSI-escaped lines to HTML
pub fn ansi_lines_to_html(lines: &[String]) -> String {
    lines
        .iter()
        .map(|line| {
            let converted = ansi_to_html(line);
            if converted.is_empty() {
                r#"<div class="ansi-line">&nbsp;</div>"#.to_string()
            } else {
                format!("<div class=\"ansi-line\">{}</div>", converted)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ansi_to_html_plain_text() {
        assert_eq!(ansi_to_html("hello world"), "hello world");
    }

    #[test]
    fn test_ansi_to_html_bold() {
        let result = ansi_to_html("\x1b[1mbold\x1b[0m");
        assert!(result.contains("font-weight:bold"));
        assert!(result.contains("bold"));
    }

    #[test]
    fn test_ansi_to_html_red_text() {
        let result = ansi_to_html("\x1b[31mred\x1b[0m");
        assert!(result.contains("color:#800000"));
        assert!(result.contains("red"));
    }

    #[test]
    fn test_ansi_to_html_html_escaped() {
        let result = ansi_to_html("<script>alert('xss')</script>");
        assert!(result.contains("&lt;script&gt;"));
        assert!(!result.contains("<script>"));
    }

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("<>&'\""), "&lt;&gt;&amp;&#039;&quot;");
    }

    #[test]
    fn test_color256_to_hex_standard() {
        assert_eq!(color256_to_hex(0), "#000000");
        assert_eq!(color256_to_hex(15), "#ffffff");
    }

    #[test]
    fn test_color256_to_hex_cube() {
        let result = color256_to_hex(16); // (0,0,0) -> black
        assert_eq!(result, "#000000");
    }

    #[test]
    fn test_ansi_lines_to_html() {
        let lines = vec!["hello".to_string(), "world".to_string()];
        let result = ansi_lines_to_html(&lines);
        assert!(result.contains("hello"));
        assert!(result.contains("world"));
        assert!(result.contains("ansi-line"));
    }
}
