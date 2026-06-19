//! Syntax highlighting utilities

use std::collections::HashMap;
use std::sync::LazyLock;

/// Format a highlighted text segment
pub type HighlightFormatter = Box<dyn Fn(&str) -> String + Send + Sync>;

/// Theme mapping scope names to formatters
pub type HighlightTheme = HashMap<String, HighlightFormatter>;

/// Options for syntax highlighting
#[derive(Default)]
pub struct HighlightOptions<'a> {
    pub language: Option<&'a str>,
    pub ignore_illegals: bool,
    pub theme: Option<&'a HighlightTheme>,
}


const SPAN_CLOSE: &str = "</span>";
const HIGHLIGHT_CLASS_PREFIX: &str = "hljs-";

/// Extract the hljs scope from a `<span class="hljs-xxx">` tag
fn get_scope_from_span_tag(tag: &str) -> Option<String> {
    // Match class="..." or class='...'
    let tag_lower = tag.to_lowercase();
    let class_markers = ["class=\""; 1];
    for prefix in class_markers {
        if let Some(start) = tag_lower.find(prefix) {
            let value_start = start + prefix.len();
            if let Some(end) = tag_lower[value_start..].find('"') {
                let class_value = &tag_lower[value_start..value_start + end];
                for class_name in class_value.split_whitespace() {
                    if let Some(scope) = class_name.strip_prefix(HIGHLIGHT_CLASS_PREFIX) {
                        return Some(scope.to_string());
                    }
                }
            }
        }
    }
    None
}

// get_scope_formatter is no longer needed — get_active_formatter
// handles exact, dot-prefix, and dash-prefix matching inline.

/// Find active formatter from scope stack
/// Returns the formatter reference (not cloned) - must be used immediately
fn get_active_formatter<'a>(
    scopes: &[Option<String>],
    theme: &'a HighlightTheme,
) -> Option<&'a HighlightFormatter> {
    for scope in scopes.iter().rev() {
        if let Some(s) = scope {
            // Exact match
            if let Some(f) = theme.get(s) {
                return Some(f);
            }
            // Dot prefix
            if let Some(dot) = s.find('.')
                && let Some(f) = theme.get(&s[..dot]) {
                    return Some(f);
                }
            // Dash prefix
            if let Some(dash) = s.find('-')
                && let Some(f) = theme.get(&s[..dash]) {
                    return Some(f);
                }
        }
    }
    theme.get("default")
}

/// Check if a position in HTML is the start of a `<span` tag
fn is_span_open_tag_start(html: &str, index: usize) -> bool {
    if !html[index..].starts_with("<span") {
        return false;
    }
    let after_span = index + "<span".len();
    html.as_bytes()
        .get(after_span)
        .is_some_and(|&c| c == b'>' || c == b' ' || c == b'\t' || c == b'\n' || c == b'\r')
}

/// Render highlighted HTML by applying theme formatters to span-wrapped text
/// Handles multi-byte UTF-8 characters safely (e.g., Chinese chars in code comments).
pub fn render_highlighted_html(html: &str, theme: &HighlightTheme) -> String {
    let mut output = String::new();
    let mut text_buffer = String::new();
    let mut scopes: Vec<Option<String>> = Vec::new();
    // Track the full text char-by-char to handle multi-byte UTF-8 safely.
    // We use `bytes` for byte-level checks and `char_indices` for char boundaries.
    let char_indices: Vec<(usize, char)> = html.char_indices().collect();

    let flush_text = |output: &mut String,
                      text_buffer: &mut String,
                      scopes: &[Option<String>],
                      theme: &HighlightTheme| {
        if text_buffer.is_empty() {
            return;
        }
        let formatter = get_active_formatter(scopes, theme);
        match formatter {
            Some(f) => output.push_str(&f(text_buffer)),
            None => output.push_str(text_buffer),
        }
        text_buffer.clear();
    };

    let bytes = html.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    // Pre-computed position for checking span close
    let span_close_start = SPAN_CLOSE.as_bytes();

    while i < len {
        // Skip if we're inside a multi-byte UTF-8 character (not at a char boundary).
        // This can happen when bytes like 0x3C ('<') or 0x26 ('&') appear inside
        // multi-byte char encodings in Chinese/special text — those bytes are NOT
        // valid char boundaries for string slicing.
        if !html.is_char_boundary(i) {
            i += 1;
            continue;
        }

        // Check for <span open tag using byte comparison (no string slice needed)
        if bytes[i] == b'<' && bytes[i..].starts_with(b"<span")
            && let Some(tag_end_offset) = html[i..].find('>') {
                let tag_end = i + tag_end_offset + 1;
                flush_text(&mut output, &mut text_buffer, &scopes, theme);
                let tag = &html[i..tag_end];
                scopes.push(get_scope_from_span_tag(tag));
                i = tag_end;
                continue;
            }

        // Check for </span> close tag using byte comparison
        if bytes[i] == b'<' && bytes[i..].starts_with(span_close_start) {
            flush_text(&mut output, &mut text_buffer, &scopes, theme);
            scopes.pop();
            i += SPAN_CLOSE.len();
            continue;
        }

        // Handle HTML entities - simplified (&amp; &lt; &gt; &quot; &#39; &#x27; &#x60; &#123; &#125;)
        if bytes[i] == b'&'
            && let Some((decoded, consumed)) = decode_html_entity_at(html, i) {
                text_buffer.push_str(&decoded);
                i += consumed;
                continue;
            }

        // Safe: push current char (handles multi-byte chars correctly since we're at a char boundary)
        if let Some((_, c)) = char_indices.iter().find(|(pos, _)| *pos == i) {
            text_buffer.push(*c);
        }
        i += 1;
    }

    flush_text(&mut output, &mut text_buffer, &mut scopes, theme);
    output
}

/// Decode a single HTML entity at position
fn decode_html_entity_at(html: &str, index: usize) -> Option<(String, usize)> {
    let remaining = &html[index..];
    if let Some(semi) = remaining.find(';') {
        let entity = &remaining[..=semi];
        let decoded = match entity {
            "&amp;" => "&",
            "&lt;" => "<",
            "&gt;" => ">",
            "&quot;" => "\"",
            "&#39;" => "'",
            "&#x27;" => "'",
            "&#x60;" => "`",
            "&#123;" => "{",
            "&#125;" => "}",
            _ => return None,
        };
        Some((decoded.to_string(), entity.len()))
    } else {
        None
    }
}

/// Highlight code using syntect
pub fn highlight(code: &str, options: &HighlightOptions) -> String {
    use syntect::easy::HighlightLines;
    use syntect::highlighting::ThemeSet;
    use syntect::html::styled_line_to_highlighted_html;
    use syntect::parsing::SyntaxSet;

    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();

    let syntax = match options.language {
        Some(lang) => ss
            .find_syntax_by_token(lang)
            .unwrap_or_else(|| ss.find_syntax_plain_text()),
        None => ss
            .find_syntax_by_first_line(code)
            .unwrap_or_else(|| ss.find_syntax_plain_text()),
    };

    let theme = &ts.themes["base16-ocean.dark"];
    let mut highlighter = HighlightLines::new(syntax, theme);

    let mut output = String::new();
    for line in code.lines() {
        if let Ok(ranges) = highlighter.highlight_line(line, &ss)
            && let Ok(html) =
                styled_line_to_highlighted_html(&ranges, syntect::html::IncludeBackground::No)
            {
                output.push_str(&html);
            }
    }

    // If a custom theme is provided, apply it to the highlighted HTML
    if let Some(theme) = options.theme
        && !theme.is_empty() {
            return render_highlighted_html(&output, theme);
        }
    output
}

/// Check if a language is supported
pub fn supports_language(name: &str) -> bool {
    let ss = syntect::parsing::SyntaxSet::load_defaults_newlines();
    ss.find_syntax_by_token(name).is_some()
}

static SYNTAX_SET: LazyLock<syntect::parsing::SyntaxSet> =
    LazyLock::new(syntect::parsing::SyntaxSet::load_defaults_newlines);

static THEME_SET: LazyLock<syntect::highlighting::ThemeSet> =
    LazyLock::new(syntect::highlighting::ThemeSet::load_defaults);

/// Build a HighlightTheme from the 9 syntax* colors.
/// Maps syntect scope categories to theme color ANSI formatters.
pub fn build_highlight_theme(
    comment: &str,
    keyword: &str,
    function: &str,
    variable: &str,
    string: &str,
    number: &str,
    r#type: &str,
    operator: &str,
    punctuation: &str,
) -> HighlightTheme {
    let mut theme: HighlightTheme = HashMap::new();

    // Helper: create a formatter closure for a given ANSI color string
    let make_fmt = |color: String| -> HighlightFormatter {
        Box::new(move |s: &str| format!("{}{}\x1b[39m", color, s))
    };

    // Comment
    theme.insert("comment".to_string(), make_fmt(comment.to_string()));

    // Keyword
    theme.insert("keyword".to_string(), make_fmt(keyword.to_string()));

    // Function / method (each entry gets its own cloned string)
    theme.insert(
        "entity.name.function".to_string(),
        make_fmt(function.to_string()),
    );
    theme.insert(
        "support.function".to_string(),
        make_fmt(function.to_string()),
    );
    theme.insert(
        "meta.function-call".to_string(),
        make_fmt(function.to_string()),
    );

    // Variable
    theme.insert("variable".to_string(), make_fmt(variable.to_string()));
    theme.insert("variable.other".to_string(), make_fmt(variable.to_string()));
    theme.insert(
        "variable.parameter".to_string(),
        make_fmt(variable.to_string()),
    );

    // String
    theme.insert("string".to_string(), make_fmt(string.to_string()));

    // Number / constant
    theme.insert("constant.numeric".to_string(), make_fmt(number.to_string()));
    theme.insert(
        "constant.language".to_string(),
        make_fmt(number.to_string()),
    );

    // Type
    theme.insert("entity.name.type".to_string(), make_fmt(r#type.to_string()));
    theme.insert("support.type".to_string(), make_fmt(r#type.to_string()));
    theme.insert("storage.type".to_string(), make_fmt(r#type.to_string()));

    // Operator
    theme.insert(
        "keyword.operator".to_string(),
        make_fmt(operator.to_string()),
    );

    // Punctuation
    theme.insert("punctuation".to_string(), make_fmt(punctuation.to_string()));
    theme.insert(
        "punctuation.definition".to_string(),
        make_fmt(punctuation.to_string()),
    );

    // Default (same as comment)
    theme.insert("default".to_string(), make_fmt(comment.to_string()));

    theme
}

/// Convert syntect style ranges to ANSI 24-bit color terminal output.
/// When no custom highlight_theme is provided, falls back to syntect's built-in base16-ocean.dark.
pub fn highlight_to_ansi(code: &str, language: Option<&str>) -> String {
    highlight_to_ansi_with_theme(code, language, None)
}

/// Same as highlight_to_ansi but accepts an optional custom HighlightTheme.
/// When a highlight_theme is provided, uses the HTML-based scope mapping.
/// Otherwise falls back to syntect's built-in base16-ocean.dark theme (direct ANSI).
pub fn highlight_to_ansi_with_theme(
    code: &str,
    language: Option<&str>,
    highlight_theme: Option<&HighlightTheme>,
) -> String {
    use syntect::easy::HighlightLines;
    use syntect::highlighting::FontStyle;
    use syntect::highlighting::Style;

    fn style_to_ansi(style: &Style, text: &str) -> String {
        let fg = &style.foreground;
        let mut out = String::new();
        out.push_str(&format!("\x1b[38;2;{};{};{}m", fg.r, fg.g, fg.b));
        if style.font_style.contains(FontStyle::BOLD) {
            out.push_str("\x1b[1m");
        }
        if style.font_style.contains(FontStyle::ITALIC) {
            out.push_str("\x1b[3m");
        }
        if style.font_style.contains(FontStyle::UNDERLINE) {
            out.push_str("\x1b[4m");
        }
        out.push_str(text);
        out.push_str("\x1b[0m");
        out
    }

    let syntax = match language {
        Some(lang) => SYNTAX_SET
            .find_syntax_by_token(lang)
            .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text()),
        None => SYNTAX_SET
            .find_syntax_by_first_line(code)
            .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text()),
    };

    // If a custom highlight theme is provided, use the HTML intermediate + scope mapping
    if let Some(theme) = highlight_theme
        && !theme.is_empty() {
            // Use syntect's HTML output then map scopes to theme colors
            let mut highlighter =
                HighlightLines::new(syntax, &THEME_SET.themes["base16-ocean.dark"]);
            use syntect::html::styled_line_to_highlighted_html;
            let mut html_output = String::new();
            for line in code.lines() {
                if let Ok(ranges) = highlighter.highlight_line(line, &SYNTAX_SET)
                    && let Ok(html) = styled_line_to_highlighted_html(
                        &ranges,
                        syntect::html::IncludeBackground::No,
                    ) {
                        html_output.push_str(&html);
                    }
            }
            return render_highlighted_html(&html_output, theme);
        }

    // Fallback: direct ANSI from syntect's built-in base16-ocean.dark theme
    let mut highlighter = HighlightLines::new(syntax, &THEME_SET.themes["base16-ocean.dark"]);

    let mut output = String::with_capacity(code.len() + code.len() / 2);
    for line in code.lines() {
        if let Ok(ranges) = highlighter.highlight_line(line, &SYNTAX_SET) {
            for (style, text) in &ranges {
                output.push_str(&style_to_ansi(style, text));
            }
            output.push('\n');
        }
    }

    output
}

/// Detect programming language from file path extension.
/// Returns None if the language is not recognized or not code.
pub fn detect_language_from_path(path: &str) -> Option<&'static str> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    // Map file extensions to syntect language tokens
    // Focus on common code file types
    match ext.to_lowercase().as_str() {
        "rs" => Some("rust"),
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "py" => Some("python"),
        "go" => Some("go"),
        "java" => Some("java"),
        "kt" | "kts" => Some("kotlin"),
        "swift" => Some("swift"),
        "rb" => Some("ruby"),
        "php" => Some("php"),
        "c" | "h" => Some("c"),
        "cpp" | "hpp" | "cc" | "cxx" => Some("cpp"),
        "cs" => Some("csharp"),
        "hs" => Some("haskell"),
        "scala" => Some("scala"),
        "sh" | "bash" | "zsh" => Some("bash"),
        "sql" => Some("sql"),
        "html" | "htm" => Some("html"),
        "css" | "scss" | "less" => Some("css"),
        "json" => Some("json"),
        "yaml" | "yml" => Some("yaml"),
        "toml" => Some("toml"),
        "md" | "markdown" => Some("markdown"),
        "xml" | "svg" => Some("xml"),
        "dockerfile" => Some("dockerfile"),
        "makefile" | "mk" => Some("makefile"),
        "cmake" => Some("cmake"),
        "lua" => Some("lua"),
        "pl" | "pm" => Some("perl"),
        "r" => Some("r"),
        "dart" => Some("dart"),
        "zig" => Some("zig"),
        "elm" => Some("elm"),
        _ => None,
    }
}
