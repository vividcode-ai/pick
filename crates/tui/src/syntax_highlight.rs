//! Code syntax highlighting via syntect.

use std::sync::OnceLock;
use std::sync::RwLock;

use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::highlighting::FontStyle;
use syntect::highlighting::Theme;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

// Guardrails
const MAX_HIGHLIGHT_BYTES: usize = 512 * 1024;
const MAX_HIGHLIGHT_LINES: usize = 10_000;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME: OnceLock<RwLock<Theme>> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme_lock() -> &'static RwLock<Theme> {
    THEME.get_or_init(|| {
        let ts = ThemeSet::load_defaults();
        RwLock::new(ts.themes["base16-ocean.dark"].clone())
    })
}

/// Set the syntax highlighting theme by name from syntect's built-in themes.
pub fn set_theme(name: &str) {
    let ts = ThemeSet::load_defaults();
    if let Some(theme) = ts.themes.get(name) {
        if let Ok(mut guard) = theme_lock().write() {
            *guard = theme.clone();
        }
    }
}

fn convert_syntect_color(sc: syntect::highlighting::Color) -> Option<Color> {
    // syntect alpha: 0 = use ANSI palette, 1 = terminal default, others = RGB
    if sc.a == 0 {
        // ANSI palette index — map to terminal named colors
        Some(match sc.r {
            0 => Color::Black,
            1 => Color::Red,
            2 => Color::Green,
            3 => Color::Yellow,
            4 => Color::Blue,
            5 => Color::Magenta,
            6 => Color::Cyan,
            7 => Color::Gray,
            8.. => Color::Indexed(sc.r),
        })
    } else if sc.a == 1 {
        None // terminal default
    } else {
        Some(Color::Rgb(sc.r, sc.g, sc.b))
    }
}

fn convert_style(syn_style: syntect::highlighting::Style) -> Style {
    let mut style = Style::default();

    // Foreground only (skip background to keep terminal bg)
    if let Some(fg) = convert_syntect_color(syn_style.foreground) {
        style = style.fg(fg);
    }

    // Bold preserved, italic/underline skipped
    if syn_style.font_style.contains(FontStyle::BOLD) {
        style = style.add_modifier(Modifier::BOLD);
    }

    style
}

/// Highlight code to styled ratatui lines.
/// Falls back to plain text on error or guardrail limits.
pub fn highlight_code_to_lines(code: &str, lang: &str) -> Vec<Line<'static>> {
    let bytes = code.len();
    let lines_count = code.lines().count();

    if bytes > MAX_HIGHLIGHT_BYTES || lines_count > MAX_HIGHLIGHT_LINES {
        return fallback_plain(code);
    }

    let ss = syntax_set();
    let syntax = ss
        .find_syntax_by_token(lang)
        .or_else(|| ss.find_syntax_by_name(lang))
        .or_else(|| ss.find_syntax_by_extension(lang))
        .or_else(|| Some(ss.find_syntax_plain_text()));

    let Some(syntax) = syntax else {
        return fallback_plain(code);
    };

    let theme = match theme_lock().read() {
        Ok(g) => g.clone(),
        Err(_) => return fallback_plain(code),
    };

    let mut highlighter = HighlightLines::new(syntax, &theme);
    let mut result = Vec::new();

    for line in code.lines() {
        match highlighter.highlight_line(line, ss) {
            Ok(regions) => {
                let mut spans = Vec::new();
                for (syn_style, text) in regions {
                    let span = Span {
                        style: convert_style(syn_style),
                        content: std::borrow::Cow::Owned(text.to_string()),
                    };
                    spans.push(span);
                }
                result.push(Line::from(spans));
            }
            Err(_) => {
                result.push(Line::from(Span::raw(line.to_string())));
            }
        }
    }

    result
}

fn fallback_plain(code: &str) -> Vec<Line<'static>> {
    code.lines()
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect()
}
