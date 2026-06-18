use std::collections::{HashMap, HashSet};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ColorValue {
    Hex(String),
    Index(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeColor {
    Accent,
    Border,
    BorderAccent,
    BorderMuted,
    Success,
    Error,
    Warning,
    Muted,
    Dim,
    Text,
    ThinkingText,
    UserMessageText,
    CustomMessageText,
    CustomMessageLabel,
    ToolTitle,
    ToolOutput,
    MdHeading,
    MdLink,
    MdLinkUrl,
    MdCode,
    MdCodeBlock,
    MdCodeBlockBorder,
    MdQuote,
    MdQuoteBorder,
    MdHr,
    MdListBullet,
    ToolDiffAdded,
    ToolDiffRemoved,
    ToolDiffContext,
    SyntaxComment,
    SyntaxKeyword,
    SyntaxFunction,
    SyntaxVariable,
    SyntaxString,
    SyntaxNumber,
    SyntaxType,
    SyntaxOperator,
    SyntaxPunctuation,
    ThinkingOff,
    ThinkingMinimal,
    ThinkingLow,
    ThinkingMedium,
    ThinkingHigh,
    ThinkingXhigh,
    BashMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeBg {
    SelectedBg,
    UserMessageBg,
    CustomMessageBg,
    ToolPendingBg,
    ToolSuccessBg,
    ToolErrorBg,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorMode {
    TrueColor,
    Ansi256,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThemeColors {
    pub accent: ColorValue,
    pub border: ColorValue,
    #[serde(rename = "borderAccent")]
    pub border_accent: ColorValue,
    #[serde(rename = "borderMuted")]
    pub border_muted: ColorValue,
    pub success: ColorValue,
    pub error: ColorValue,
    pub warning: ColorValue,
    pub muted: ColorValue,
    pub dim: ColorValue,
    pub text: ColorValue,
    #[serde(rename = "thinkingText")]
    pub thinking_text: ColorValue,
    #[serde(rename = "selectedBg")]
    pub selected_bg: ColorValue,
    #[serde(rename = "userMessageBg")]
    pub user_message_bg: ColorValue,
    #[serde(rename = "userMessageText")]
    pub user_message_text: ColorValue,
    #[serde(rename = "customMessageBg")]
    pub custom_message_bg: ColorValue,
    #[serde(rename = "customMessageText")]
    pub custom_message_text: ColorValue,
    #[serde(rename = "customMessageLabel")]
    pub custom_message_label: ColorValue,
    #[serde(rename = "toolPendingBg")]
    pub tool_pending_bg: ColorValue,
    #[serde(rename = "toolSuccessBg")]
    pub tool_success_bg: ColorValue,
    #[serde(rename = "toolErrorBg")]
    pub tool_error_bg: ColorValue,
    #[serde(rename = "toolTitle")]
    pub tool_title: ColorValue,
    #[serde(rename = "toolOutput")]
    pub tool_output: ColorValue,
    #[serde(rename = "mdHeading")]
    pub md_heading: ColorValue,
    #[serde(rename = "mdLink")]
    pub md_link: ColorValue,
    #[serde(rename = "mdLinkUrl")]
    pub md_link_url: ColorValue,
    #[serde(rename = "mdCode")]
    pub md_code: ColorValue,
    #[serde(rename = "mdCodeBlock")]
    pub md_code_block: ColorValue,
    #[serde(rename = "mdCodeBlockBorder")]
    pub md_code_block_border: ColorValue,
    #[serde(rename = "mdQuote")]
    pub md_quote: ColorValue,
    #[serde(rename = "mdQuoteBorder")]
    pub md_quote_border: ColorValue,
    #[serde(rename = "mdHr")]
    pub md_hr: ColorValue,
    #[serde(rename = "mdListBullet")]
    pub md_list_bullet: ColorValue,
    #[serde(rename = "toolDiffAdded")]
    pub tool_diff_added: ColorValue,
    #[serde(rename = "toolDiffRemoved")]
    pub tool_diff_removed: ColorValue,
    #[serde(rename = "toolDiffContext")]
    pub tool_diff_context: ColorValue,
    #[serde(rename = "syntaxComment")]
    pub syntax_comment: ColorValue,
    #[serde(rename = "syntaxKeyword")]
    pub syntax_keyword: ColorValue,
    #[serde(rename = "syntaxFunction")]
    pub syntax_function: ColorValue,
    #[serde(rename = "syntaxVariable")]
    pub syntax_variable: ColorValue,
    #[serde(rename = "syntaxString")]
    pub syntax_string: ColorValue,
    #[serde(rename = "syntaxNumber")]
    pub syntax_number: ColorValue,
    #[serde(rename = "syntaxType")]
    pub syntax_type: ColorValue,
    #[serde(rename = "syntaxOperator")]
    pub syntax_operator: ColorValue,
    #[serde(rename = "syntaxPunctuation")]
    pub syntax_punctuation: ColorValue,
    #[serde(rename = "thinkingOff")]
    pub thinking_off: ColorValue,
    #[serde(rename = "thinkingMinimal")]
    pub thinking_minimal: ColorValue,
    #[serde(rename = "thinkingLow")]
    pub thinking_low: ColorValue,
    #[serde(rename = "thinkingMedium")]
    pub thinking_medium: ColorValue,
    #[serde(rename = "thinkingHigh")]
    pub thinking_high: ColorValue,
    #[serde(rename = "thinkingXhigh")]
    pub thinking_xhigh: ColorValue,
    #[serde(rename = "bashMode")]
    pub bash_mode: ColorValue,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThemeExportColors {
    #[serde(rename = "pageBg")]
    pub page_bg: Option<ColorValue>,
    #[serde(rename = "cardBg")]
    pub card_bg: Option<ColorValue>,
    #[serde(rename = "infoBg")]
    pub info_bg: Option<ColorValue>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThemeJson {
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    pub name: String,
    #[serde(default)]
    pub vars: HashMap<String, ColorValue>,
    pub colors: ThemeColors,
    #[serde(default)]
    pub export: Option<ThemeExportColors>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Rgb {
    pub(super) r: u8,
    pub(super) g: u8,
    pub(super) b: u8,
}

pub(super) fn hex_to_rgb(hex: &str) -> Option<Rgb> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Rgb { r, g, b })
}

const CUBE_VALUES: [u8; 6] = [0, 95, 135, 175, 215, 255];

const GRAY_VALUES: [u8; 24] = [
    8, 18, 28, 38, 48, 58, 68, 78, 88, 98, 108, 118, 128, 138, 148, 158, 168, 178, 188, 198, 208,
    218, 228, 238,
];

fn find_closest_cube_index(value: u8) -> usize {
    let mut min_dist = u16::MAX;
    let mut min_idx = 0;
    for (i, &cv) in CUBE_VALUES.iter().enumerate() {
        let dist = (value as i16 - cv as i16).unsigned_abs();
        if dist < min_dist {
            min_dist = dist;
            min_idx = i;
        }
    }
    min_idx
}

fn find_closest_gray_index(gray: u8) -> usize {
    let mut min_dist = u16::MAX;
    let mut min_idx = 0;
    for (i, &gv) in GRAY_VALUES.iter().enumerate() {
        let dist = (gray as i16 - gv as i16).unsigned_abs();
        if dist < min_dist {
            min_dist = dist;
            min_idx = i;
        }
    }
    min_idx
}

fn color_distance(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> f64 {
    let dr = r1 as f64 - r2 as f64;
    let dg = g1 as f64 - g2 as f64;
    let db = b1 as f64 - b2 as f64;
    dr * dr * 0.299 + dg * dg * 0.587 + db * db * 0.114
}

fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    let r_idx = find_closest_cube_index(r);
    let g_idx = find_closest_cube_index(g);
    let b_idx = find_closest_cube_index(b);
    let cube_r = CUBE_VALUES[r_idx];
    let cube_g = CUBE_VALUES[g_idx];
    let cube_b = CUBE_VALUES[b_idx];
    let cube_index: u8 = 16 + 36 * r_idx as u8 + 6 * g_idx as u8 + b_idx as u8;
    let cube_dist = color_distance(r, g, b, cube_r, cube_g, cube_b);

    let gray = (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64).round() as u8;
    let gray_idx = find_closest_gray_index(gray);
    let gray_value = GRAY_VALUES[gray_idx];
    let gray_index: u8 = 232 + gray_idx as u8;
    let gray_dist = color_distance(r, g, b, gray_value, gray_value, gray_value);

    let max_c = r.max(g).max(b);
    let min_c = r.min(g).min(b);
    let spread = max_c - min_c;

    if spread < 10 && gray_dist < cube_dist {
        gray_index
    } else {
        cube_index
    }
}

pub(super) fn hex_to_256(hex: &str) -> Option<u8> {
    let rgb = hex_to_rgb(hex)?;
    Some(rgb_to_256(rgb.r, rgb.g, rgb.b))
}

fn fg_ansi(color: &ResolvedColor, mode: ColorMode) -> String {
    match (color, mode) {
        (ResolvedColor::Empty, _) => "\x1b[39m".to_string(),
        (ResolvedColor::Index(idx), _) => format!("\x1b[38;5;{}m", idx),
        (ResolvedColor::Hex(hex), ColorMode::TrueColor) => {
            if let Some(rgb) = hex_to_rgb(hex) {
                format!("\x1b[38;2;{};{};{}m", rgb.r, rgb.g, rgb.b)
            } else {
                "\x1b[39m".to_string()
            }
        }
        (ResolvedColor::Hex(hex), ColorMode::Ansi256) => {
            if let Some(idx) = hex_to_256(hex) {
                format!("\x1b[38;5;{}m", idx)
            } else {
                "\x1b[39m".to_string()
            }
        }
    }
}

fn bg_ansi(color: &ResolvedColor, mode: ColorMode) -> String {
    match (color, mode) {
        (ResolvedColor::Empty, _) => "\x1b[49m".to_string(),
        (ResolvedColor::Index(idx), _) => format!("\x1b[48;5;{}m", idx),
        (ResolvedColor::Hex(hex), ColorMode::TrueColor) => {
            if let Some(rgb) = hex_to_rgb(hex) {
                format!("\x1b[48;2;{};{};{}m", rgb.r, rgb.g, rgb.b)
            } else {
                "\x1b[49m".to_string()
            }
        }
        (ResolvedColor::Hex(hex), ColorMode::Ansi256) => {
            if let Some(idx) = hex_to_256(hex) {
                format!("\x1b[48;5;{}m", idx)
            } else {
                "\x1b[49m".to_string()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum ResolvedColor {
    Empty,
    Index(u8),
    Hex(String),
}

fn resolve_var_refs(
    value: &ColorValue,
    vars: &HashMap<String, ColorValue>,
    visited: &mut HashSet<String>,
) -> Result<ResolvedColor, String> {
    match value {
        ColorValue::Index(idx) => Ok(ResolvedColor::Index(*idx as u8)),
        ColorValue::Hex(h) if h.is_empty() || h.starts_with('#') => {
            if h.is_empty() {
                Ok(ResolvedColor::Empty)
            } else {
                Ok(ResolvedColor::Hex(h.clone()))
            }
        }
        ColorValue::Hex(var_name) => {
            if visited.contains(var_name) {
                return Err(format!("Circular variable reference detected: {}", var_name));
            }
            let resolved = vars.get(var_name).ok_or_else(|| {
                format!("Variable reference not found: {}", var_name)
            })?;
            visited.insert(var_name.clone());
            resolve_var_refs(resolved, vars, visited)
        }
    }
}

fn resolve_color_value(
    value: &ColorValue,
    vars: &HashMap<String, ColorValue>,
) -> Result<ResolvedColor, String> {
    let mut visited = HashSet::new();
    resolve_var_refs(value, vars, &mut visited)
}

const BG_COLOR_KEYS: &[&str] = &[
    "selectedBg",
    "userMessageBg",
    "customMessageBg",
    "toolPendingBg",
    "toolSuccessBg",
    "toolErrorBg",
];

pub(super) fn color_value_to_resolved(value: &ColorValue, vars: &HashMap<String, ColorValue>) -> ResolvedColor {
    resolve_color_value(value, vars).unwrap_or(ResolvedColor::Empty)
}

pub struct Theme {
    pub name: Option<String>,
    pub source_path: Option<String>,
    fg_colors: HashMap<ThemeColor, String>,
    bg_colors: HashMap<ThemeBg, String>,
    resolved_fg: HashMap<ThemeColor, ResolvedColor>,
    resolved_bg: HashMap<ThemeBg, ResolvedColor>,
    mode: ColorMode,
}

impl Theme {
    pub fn new(
        name: Option<String>,
        source_path: Option<String>,
        theme_json: &ThemeJson,
        mode: ColorMode,
    ) -> Self {
        let mut fg_colors = HashMap::new();
        let mut bg_colors = HashMap::new();
        let mut resolved_fg = HashMap::new();
        let mut resolved_bg = HashMap::new();

        let colors = &theme_json.colors;
        let vars = &theme_json.vars;

        let all_colors: Vec<(&str, &ColorValue)> = vec![
            ("accent", &colors.accent),
            ("border", &colors.border),
            ("borderAccent", &colors.border_accent),
            ("borderMuted", &colors.border_muted),
            ("success", &colors.success),
            ("error", &colors.error),
            ("warning", &colors.warning),
            ("muted", &colors.muted),
            ("dim", &colors.dim),
            ("text", &colors.text),
            ("thinkingText", &colors.thinking_text),
            ("selectedBg", &colors.selected_bg),
            ("userMessageBg", &colors.user_message_bg),
            ("userMessageText", &colors.user_message_text),
            ("customMessageBg", &colors.custom_message_bg),
            ("customMessageText", &colors.custom_message_text),
            ("customMessageLabel", &colors.custom_message_label),
            ("toolPendingBg", &colors.tool_pending_bg),
            ("toolSuccessBg", &colors.tool_success_bg),
            ("toolErrorBg", &colors.tool_error_bg),
            ("toolTitle", &colors.tool_title),
            ("toolOutput", &colors.tool_output),
            ("mdHeading", &colors.md_heading),
            ("mdLink", &colors.md_link),
            ("mdLinkUrl", &colors.md_link_url),
            ("mdCode", &colors.md_code),
            ("mdCodeBlock", &colors.md_code_block),
            ("mdCodeBlockBorder", &colors.md_code_block_border),
            ("mdQuote", &colors.md_quote),
            ("mdQuoteBorder", &colors.md_quote_border),
            ("mdHr", &colors.md_hr),
            ("mdListBullet", &colors.md_list_bullet),
            ("toolDiffAdded", &colors.tool_diff_added),
            ("toolDiffRemoved", &colors.tool_diff_removed),
            ("toolDiffContext", &colors.tool_diff_context),
            ("syntaxComment", &colors.syntax_comment),
            ("syntaxKeyword", &colors.syntax_keyword),
            ("syntaxFunction", &colors.syntax_function),
            ("syntaxVariable", &colors.syntax_variable),
            ("syntaxString", &colors.syntax_string),
            ("syntaxNumber", &colors.syntax_number),
            ("syntaxType", &colors.syntax_type),
            ("syntaxOperator", &colors.syntax_operator),
            ("syntaxPunctuation", &colors.syntax_punctuation),
            ("thinkingOff", &colors.thinking_off),
            ("thinkingMinimal", &colors.thinking_minimal),
            ("thinkingLow", &colors.thinking_low),
            ("thinkingMedium", &colors.thinking_medium),
            ("thinkingHigh", &colors.thinking_high),
            ("thinkingXhigh", &colors.thinking_xhigh),
            ("bashMode", &colors.bash_mode),
        ];

        for (key, value) in all_colors {
            let resolved = color_value_to_resolved(value, vars);
            let is_bg = BG_COLOR_KEYS.contains(&key);
            if is_bg {
                let ansi = bg_ansi(&resolved, mode);
                if let Some(bg_key) = bg_key_from_str(key) {
                    bg_colors.insert(bg_key, ansi);
                    resolved_bg.insert(bg_key, resolved);
                }
            } else {
                let ansi = fg_ansi(&resolved, mode);
                if let Some(fg_key) = fg_key_from_str(key) {
                    fg_colors.insert(fg_key, ansi);
                    resolved_fg.insert(fg_key, resolved);
                }
            }
        }

        Self {
            name,
            source_path,
            fg_colors,
            bg_colors,
            resolved_fg,
            resolved_bg,
            mode,
        }
    }

    pub fn fg(&self, color: ThemeColor, text: &str) -> String {
        let ansi = self.fg_colors.get(&color).expect("Unknown theme color");
        format!("{}{}\x1b[39m", ansi, text)
    }

    pub fn bg(&self, color: ThemeBg, text: &str) -> String {
        let ansi = self.bg_colors.get(&color).expect("Unknown theme background color");
        format!("{}{}\x1b[49m", ansi, text)
    }

    pub fn bold(&self, text: &str) -> String {
        format!("\x1b[1m{}\x1b[22m", text)
    }

    pub fn italic(&self, text: &str) -> String {
        format!("\x1b[3m{}\x1b[23m", text)
    }

    pub fn underline(&self, text: &str) -> String {
        format!("\x1b[4m{}\x1b[24m", text)
    }

    pub fn inverse(&self, text: &str) -> String {
        format!("\x1b[7m{}\x1b[27m", text)
    }

    pub fn strikethrough(&self, text: &str) -> String {
        format!("\x1b[9m{}\x1b[29m", text)
    }

    pub fn get_fg_ansi(&self, color: ThemeColor) -> &str {
        self.fg_colors.get(&color).expect("Unknown theme color")
    }

    pub fn get_bg_ansi(&self, color: ThemeBg) -> &str {
        self.bg_colors.get(&color).expect("Unknown theme background color")
    }

    pub fn color_mode(&self) -> ColorMode {
        self.mode
    }

    pub fn get_thinking_border_color(&self, level: &str) -> Box<dyn Fn(&str) -> String + '_> {
        let color = match level {
            "off" => ThemeColor::ThinkingOff,
            "minimal" => ThemeColor::ThinkingMinimal,
            "low" => ThemeColor::ThinkingLow,
            "medium" => ThemeColor::ThinkingMedium,
            "high" => ThemeColor::ThinkingHigh,
            "xhigh" => ThemeColor::ThinkingXhigh,
            _ => ThemeColor::ThinkingOff,
        };
        Box::new(move |s: &str| self.fg(color, s))
    }

    pub fn get_bash_mode_border_color(&self) -> Box<dyn Fn(&str) -> String + '_> {
        Box::new(move |s: &str| self.fg(ThemeColor::BashMode, s))
    }
}

fn fg_key_from_str(s: &str) -> Option<ThemeColor> {
    match s {
        "accent" => Some(ThemeColor::Accent),
        "border" => Some(ThemeColor::Border),
        "borderAccent" => Some(ThemeColor::BorderAccent),
        "borderMuted" => Some(ThemeColor::BorderMuted),
        "success" => Some(ThemeColor::Success),
        "error" => Some(ThemeColor::Error),
        "warning" => Some(ThemeColor::Warning),
        "muted" => Some(ThemeColor::Muted),
        "dim" => Some(ThemeColor::Dim),
        "text" => Some(ThemeColor::Text),
        "thinkingText" => Some(ThemeColor::ThinkingText),
        "userMessageText" => Some(ThemeColor::UserMessageText),
        "customMessageText" => Some(ThemeColor::CustomMessageText),
        "customMessageLabel" => Some(ThemeColor::CustomMessageLabel),
        "toolTitle" => Some(ThemeColor::ToolTitle),
        "toolOutput" => Some(ThemeColor::ToolOutput),
        "mdHeading" => Some(ThemeColor::MdHeading),
        "mdLink" => Some(ThemeColor::MdLink),
        "mdLinkUrl" => Some(ThemeColor::MdLinkUrl),
        "mdCode" => Some(ThemeColor::MdCode),
        "mdCodeBlock" => Some(ThemeColor::MdCodeBlock),
        "mdCodeBlockBorder" => Some(ThemeColor::MdCodeBlockBorder),
        "mdQuote" => Some(ThemeColor::MdQuote),
        "mdQuoteBorder" => Some(ThemeColor::MdQuoteBorder),
        "mdHr" => Some(ThemeColor::MdHr),
        "mdListBullet" => Some(ThemeColor::MdListBullet),
        "toolDiffAdded" => Some(ThemeColor::ToolDiffAdded),
        "toolDiffRemoved" => Some(ThemeColor::ToolDiffRemoved),
        "toolDiffContext" => Some(ThemeColor::ToolDiffContext),
        "syntaxComment" => Some(ThemeColor::SyntaxComment),
        "syntaxKeyword" => Some(ThemeColor::SyntaxKeyword),
        "syntaxFunction" => Some(ThemeColor::SyntaxFunction),
        "syntaxVariable" => Some(ThemeColor::SyntaxVariable),
        "syntaxString" => Some(ThemeColor::SyntaxString),
        "syntaxNumber" => Some(ThemeColor::SyntaxNumber),
        "syntaxType" => Some(ThemeColor::SyntaxType),
        "syntaxOperator" => Some(ThemeColor::SyntaxOperator),
        "syntaxPunctuation" => Some(ThemeColor::SyntaxPunctuation),
        "thinkingOff" => Some(ThemeColor::ThinkingOff),
        "thinkingMinimal" => Some(ThemeColor::ThinkingMinimal),
        "thinkingLow" => Some(ThemeColor::ThinkingLow),
        "thinkingMedium" => Some(ThemeColor::ThinkingMedium),
        "thinkingHigh" => Some(ThemeColor::ThinkingHigh),
        "thinkingXhigh" => Some(ThemeColor::ThinkingXhigh),
        "bashMode" => Some(ThemeColor::BashMode),
        _ => None,
    }
}

fn bg_key_from_str(s: &str) -> Option<ThemeBg> {
    match s {
        "selectedBg" => Some(ThemeBg::SelectedBg),
        "userMessageBg" => Some(ThemeBg::UserMessageBg),
        "customMessageBg" => Some(ThemeBg::CustomMessageBg),
        "toolPendingBg" => Some(ThemeBg::ToolPendingBg),
        "toolSuccessBg" => Some(ThemeBg::ToolSuccessBg),
        "toolErrorBg" => Some(ThemeBg::ToolErrorBg),
        _ => None,
    }
}

pub fn ansi256_to_hex(index: u8) -> String {
    const BASIC_COLORS: [&str; 16] = [
        "#000000", "#800000", "#008000", "#808000", "#000080", "#800080", "#008080", "#c0c0c0",
        "#808080", "#ff0000", "#00ff00", "#ffff00", "#0000ff", "#ff00ff", "#00ffff", "#ffffff",
    ];
    if (index as usize) < 16 {
        return BASIC_COLORS[index as usize].to_string();
    }

    if index < 232 {
        let cube_index = index - 16;
        let r = cube_index / 36;
        let g = (cube_index % 36) / 6;
        let b = cube_index % 6;
        let to_hex = |n: u8| -> String {
            let val: u8 = if n == 0 { 0 } else { 55 + n * 40 };
            format!("{:02x}", val)
        };
        return format!("#{}{}{}", to_hex(r), to_hex(g), to_hex(b));
    }

    let gray = 8 + (index - 232) * 10;
    format!("#{:02x}{:02x}{:02x}", gray, gray, gray)
}

pub fn get_rgb_luminance(r: u8, g: u8, b: u8) -> f64 {
    let to_linear = |channel: f64| -> f64 {
        let value = channel / 255.0;
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * to_linear(r as f64) + 0.7152 * to_linear(g as f64) + 0.0722 * to_linear(b as f64)
}

pub struct MarkdownTheme<'a> {
    pub heading: Box<dyn Fn(&str) -> String + 'a>,
    pub link: Box<dyn Fn(&str) -> String + 'a>,
    pub link_url: Box<dyn Fn(&str) -> String + 'a>,
    pub code: Box<dyn Fn(&str) -> String + 'a>,
    pub code_block: Box<dyn Fn(&str) -> String + 'a>,
    pub code_block_border: Box<dyn Fn(&str) -> String + 'a>,
    pub quote: Box<dyn Fn(&str) -> String + 'a>,
    pub quote_border: Box<dyn Fn(&str) -> String + 'a>,
    pub hr: Box<dyn Fn(&str) -> String + 'a>,
    pub list_bullet: Box<dyn Fn(&str) -> String + 'a>,
    pub bold: Box<dyn Fn(&str) -> String + 'a>,
    pub italic: Box<dyn Fn(&str) -> String + 'a>,
    pub underline: Box<dyn Fn(&str) -> String + 'a>,
    pub strikethrough: Box<dyn Fn(&str) -> String + 'a>,
}

pub fn get_markdown_theme<'a>(theme: &'a Theme) -> MarkdownTheme<'a> {
    MarkdownTheme {
        heading: Box::new(move |s| theme.fg(ThemeColor::MdHeading, s)),
        link: Box::new(move |s| theme.fg(ThemeColor::MdLink, s)),
        link_url: Box::new(move |s| theme.fg(ThemeColor::MdLinkUrl, s)),
        code: Box::new(move |s| theme.fg(ThemeColor::MdCode, s)),
        code_block: Box::new(move |s| theme.fg(ThemeColor::MdCodeBlock, s)),
        code_block_border: Box::new(move |s| theme.fg(ThemeColor::MdCodeBlockBorder, s)),
        quote: Box::new(move |s| theme.fg(ThemeColor::MdQuote, s)),
        quote_border: Box::new(move |s| theme.fg(ThemeColor::MdQuoteBorder, s)),
        hr: Box::new(move |s| theme.fg(ThemeColor::MdHr, s)),
        list_bullet: Box::new(move |s| theme.fg(ThemeColor::MdListBullet, s)),
        bold: Box::new(move |s| theme.bold(s)),
        italic: Box::new(move |s| theme.italic(s)),
        underline: Box::new(move |s| theme.underline(s)),
        strikethrough: Box::new(move |s| theme.strikethrough(s)),
    }
}

pub struct SelectListTheme<'a> {
    pub selected_prefix: Box<dyn Fn(&str) -> String + 'a>,
    pub selected_text: Box<dyn Fn(&str) -> String + 'a>,
    pub description: Box<dyn Fn(&str) -> String + 'a>,
    pub scroll_info: Box<dyn Fn(&str) -> String + 'a>,
    pub no_match: Box<dyn Fn(&str) -> String + 'a>,
}

pub fn get_select_list_theme<'a>(theme: &'a Theme) -> SelectListTheme<'a> {
    SelectListTheme {
        selected_prefix: Box::new(move |s| theme.fg(ThemeColor::Accent, s)),
        selected_text: Box::new(move |s| theme.fg(ThemeColor::Accent, s)),
        description: Box::new(move |s| theme.fg(ThemeColor::Muted, s)),
        scroll_info: Box::new(move |s| theme.fg(ThemeColor::Muted, s)),
        no_match: Box::new(move |s| theme.fg(ThemeColor::Muted, s)),
    }
}

fn ansi_to_color(ansi: &str, default: ratatui::style::Color) -> ratatui::style::Color {
    let bytes = ansi.as_bytes();
    if bytes.len() < 12 || bytes[0] != 0x1b || bytes[1] != b'[' {
        return default;
    }
    let parts: Vec<&str> = ansi.trim_end_matches('m')
        .split(';')
        .collect();
    if parts.len() >= 3 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            parts[parts.len() - 3].parse::<u8>(),
            parts[parts.len() - 2].parse::<u8>(),
            parts[parts.len() - 1].parse::<u8>(),
        ) {
            return ratatui::style::Color::Rgb(r, g, b);
        }
    }
    default
}

pub fn theme_to_tui_colors(theme: &Theme) -> pick_tui::components::theme::TuiColors {
    use pick_tui::components::theme::TuiColors;
    use ratatui::style::Color;

    let fg = |c: ThemeColor, fallback: (u8, u8, u8)| -> Option<Color> {
        Some(ansi_to_color(theme.get_fg_ansi(c), Color::Rgb(fallback.0, fallback.1, fallback.2)))
    };
    let bg = |c: ThemeBg, fallback: (u8, u8, u8)| -> Option<Color> {
        Some(ansi_to_color(theme.get_bg_ansi(c), Color::Rgb(fallback.0, fallback.1, fallback.2)))
    };

    TuiColors {
        user_msg_bg: bg(ThemeBg::UserMessageBg, (52, 53, 69)),
        tool_pending_bg: bg(ThemeBg::ToolPendingBg, (40, 40, 50)),
        tool_success_bg: bg(ThemeBg::ToolSuccessBg, (40, 50, 40)),
        tool_error_bg: bg(ThemeBg::ToolErrorBg, (60, 40, 40)),
        text: fg(ThemeColor::Text, (212, 212, 212)),
        accent: fg(ThemeColor::Accent, (138, 190, 183)),
        muted: fg(ThemeColor::Muted, (128, 128, 128)),
        dim: fg(ThemeColor::Dim, (102, 102, 102)),
        thinking_text: fg(ThemeColor::ThinkingText, (128, 128, 128)),
        tool_title: fg(ThemeColor::ToolTitle, (212, 212, 212)),
        tool_output: fg(ThemeColor::ToolOutput, (128, 128, 128)),
        error: fg(ThemeColor::Error, (204, 102, 102)),
        md_heading: fg(ThemeColor::MdHeading, (240, 198, 116)),
        md_link: fg(ThemeColor::MdLink, (129, 162, 190)),
        md_link_url: fg(ThemeColor::MdLinkUrl, (102, 102, 102)),
        md_code: fg(ThemeColor::MdCode, (138, 190, 183)),
        md_code_block: fg(ThemeColor::MdCodeBlock, (181, 189, 104)),
        md_code_block_border: fg(ThemeColor::MdCodeBlockBorder, (128, 128, 128)),
        md_quote: fg(ThemeColor::MdQuote, (128, 128, 128)),
        md_quote_border: fg(ThemeColor::MdQuoteBorder, (128, 128, 128)),
        md_hr: fg(ThemeColor::MdHr, (128, 128, 128)),
        md_list_bullet: fg(ThemeColor::MdListBullet, (138, 190, 183)),
    }
}

pub fn get_language_from_path(file_path: &str) -> Option<&'static str> {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())?;

    match ext.as_str() {
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "py" => Some("python"),
        "rb" => Some("ruby"),
        "rs" => Some("rust"),
        "go" => Some("go"),
        "java" => Some("java"),
        "kt" => Some("kotlin"),
        "swift" => Some("swift"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" => Some("cpp"),
        "cs" => Some("csharp"),
        "php" => Some("php"),
        "sh" | "bash" | "zsh" => Some("bash"),
        "fish" => Some("fish"),
        "ps1" => Some("powershell"),
        "sql" => Some("sql"),
        "html" | "htm" => Some("html"),
        "css" => Some("css"),
        "scss" => Some("scss"),
        "sass" => Some("sass"),
        "less" => Some("less"),
        "json" => Some("json"),
        "yaml" | "yml" => Some("yaml"),
        "toml" => Some("toml"),
        "xml" => Some("xml"),
        "md" | "markdown" => Some("markdown"),
        "dockerfile" => Some("dockerfile"),
        "makefile" => Some("makefile"),
        "cmake" => Some("cmake"),
        "lua" => Some("lua"),
        "perl" => Some("perl"),
        "r" => Some("r"),
        "scala" => Some("scala"),
        "clj" => Some("clojure"),
        "ex" | "exs" => Some("elixir"),
        "erl" => Some("erlang"),
        "hs" => Some("haskell"),
        "ml" => Some("ocaml"),
        "vim" => Some("vim"),
        "graphql" => Some("graphql"),
        "proto" => Some("protobuf"),
        "tf" | "hcl" => Some("hcl"),
        _ => None,
    }
}
