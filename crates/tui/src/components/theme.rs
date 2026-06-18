//! Theme color definitions for TUI components — using native ratatui Color values.

use ratatui::style::Color;

/// All theme colors used by TUI components.
/// Uses native ratatui Color instead of ANSI escape strings.
#[derive(Clone)]
pub struct TuiColors {
    // Backgrounds
    pub user_msg_bg: Option<Color>,
    pub tool_pending_bg: Option<Color>,
    pub tool_success_bg: Option<Color>,
    pub tool_error_bg: Option<Color>,

    // Foregrounds
    pub text: Option<Color>,
    pub accent: Option<Color>,
    pub muted: Option<Color>,
    pub dim: Option<Color>,
    pub thinking_text: Option<Color>,
    pub tool_title: Option<Color>,
    pub tool_output: Option<Color>,
    pub error: Option<Color>,

    // Markdown colors
    pub md_heading: Option<Color>,
    pub md_link: Option<Color>,
    pub md_link_url: Option<Color>,
    pub md_code: Option<Color>,
    pub md_code_block: Option<Color>,
    pub md_code_block_border: Option<Color>,
    pub md_quote: Option<Color>,
    pub md_quote_border: Option<Color>,
    pub md_hr: Option<Color>,
    pub md_list_bullet: Option<Color>,
}

impl TuiColors {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for TuiColors {
    fn default() -> Self {
        use Color::Rgb;
        Self {
            // Dark theme backgrounds
            user_msg_bg: Some(Rgb(52, 53, 69)),       // #343541
            tool_pending_bg: Some(Rgb(40, 40, 50)),    // #282832
            tool_success_bg: Some(Rgb(40, 50, 40)),    // #283228
            tool_error_bg: Some(Rgb(60, 40, 40)),      // #3c2828

            // Dark theme foregrounds
            text: Some(Rgb(212, 212, 212)),              // #d4d4d4
            accent: Some(Rgb(138, 190, 183)),            // #8abeb7
            muted: Some(Rgb(128, 128, 128)),             // #808080
            dim: Some(Rgb(102, 102, 102)),               // #666666
            thinking_text: Some(Rgb(128, 128, 128)),     // #808080
            tool_title: Some(Rgb(212, 212, 212)),        // #d4d4d4
            tool_output: Some(Rgb(128, 128, 128)),       // #808080
            error: Some(Rgb(204, 102, 102)),             // #cc6666

            // Dark theme markdown
            md_heading: Some(Rgb(240, 198, 116)),         // #f0c674
            md_link: Some(Rgb(129, 162, 190)),            // #81a2be
            md_link_url: Some(Rgb(102, 102, 102)),        // #666666
            md_code: Some(Rgb(138, 190, 183)),            // #8abeb7
            md_code_block: Some(Rgb(181, 189, 104)),      // #b5bd68
            md_code_block_border: Some(Rgb(128, 128, 128)), // #808080
            md_quote: Some(Rgb(128, 128, 128)),           // #808080
            md_quote_border: Some(Rgb(128, 128, 128)),    // #808080
            md_hr: Some(Rgb(128, 128, 128)),              // #808080
            md_list_bullet: Some(Rgb(138, 190, 183)),     // #8abeb7
        }
    }
}
