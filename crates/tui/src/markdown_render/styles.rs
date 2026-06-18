use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;

#[derive(Clone, Debug)]
pub(crate) struct MarkdownStyles {
    pub heading: [Style; 6],
    pub bold: Style,
    pub italic: Style,
    pub strikethrough: Style,
    pub code: Style,
    pub code_block: Style,
    pub code_block_border: Style,
    pub quote: Style,
    pub quote_border: Style,
    pub link: Style,
    pub link_url: Style,
    pub list_bullet: Style,
    pub hr: Style,
}

impl Default for MarkdownStyles {
    fn default() -> Self {
        Self {
            heading: [
                Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                Style::default().add_modifier(Modifier::BOLD),
                Style::default().add_modifier(Modifier::BOLD | Modifier::ITALIC),
                Style::default().add_modifier(Modifier::ITALIC),
                Style::default().add_modifier(Modifier::ITALIC),
                Style::default().add_modifier(Modifier::ITALIC),
            ],
            bold: Style::default().add_modifier(Modifier::BOLD),
            italic: Style::default().add_modifier(Modifier::ITALIC),
            strikethrough: Style::default().add_modifier(Modifier::CROSSED_OUT),
            code: Style::default().fg(Color::Cyan),
            code_block: Style::default().fg(Color::Cyan),
            code_block_border: Style::default().add_modifier(Modifier::DIM),
            quote: Style::default().fg(Color::Green),
            quote_border: Style::default().fg(Color::Green),
            link: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::UNDERLINED),
            link_url: Style::default().fg(Color::Cyan),
            list_bullet: Style::default(),
            hr: Style::default(),
        }
    }
}
