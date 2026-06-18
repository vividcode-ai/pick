//! Update prompt dialog component for the TUI.

#[derive(Debug, Clone)]
pub struct UpdatePromptState {
    pub current_version: String,
    pub new_version: String,
    pub selected: usize,
}

impl UpdatePromptState {
    pub fn new(current_version: &str, new_version: &str) -> Self {
        Self {
            current_version: current_version.to_string(),
            new_version: new_version.to_string(),
            selected: 0,
        }
    }

    pub fn previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn next(&mut self) {
        self.selected = self.selected.saturating_add(1).min(2);
    }

    pub fn render(&self, width: u16) -> Vec<ratatui::text::Line<'static>> {
        use ratatui::prelude::*;
        let width = width as usize;
        let mut lines: Vec<Line> = Vec::new();

        let dim = Style::default().add_modifier(Modifier::DIM);
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let accent = Style::default().fg(Color::Cyan);
        let selected_bg = Style::default().bg(Color::Rgb(50, 80, 120)).fg(Color::White);

        let sep = "\u{2500}".repeat(width);

        // Top separator
        lines.push(Line::from(Span::styled(sep.clone(), dim)));
        lines.push(Line::from(""));

        // Title
        lines.push(Line::from(Span::styled("  Update Available", bold)));
        lines.push(Line::from(""));

        // Version info
        let info = format!(
            "  Pick v{}  →  v{}",
            self.current_version,
            self.new_version,
        );
        lines.push(Line::from(Span::styled(info, Style::default())));
        lines.push(Line::from(""));

        // Options
        let options = [
            ("  1. Update now", "Download and install the latest version"),
            ("  2. Skip", "Remind me later"),
            ("  3. Don't remind for this version", "Silence notifications for v{}"),
        ];

        for (i, (label, desc)) in options.iter().enumerate() {
            let is_selected = i == self.selected;
            let desc = if i == 2 {
                desc.replace("{}", &self.new_version)
            } else {
                desc.to_string()
            };

            if is_selected {
                let marker = "\u{276f}"; // ❯
                let line = format!("  {} {}  {}", marker, label, desc);
                lines.push(Line::from(Span::styled(line, selected_bg)));
            } else {
                let marker = " ";
                let line = format!("  {} {}  {}", marker, label, desc);
                lines.push(Line::from(Span::styled(line, Style::default())));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  \u{2191}\u{2193} or j/k navigate  ", dim),
            Span::styled("Enter", accent),
            Span::styled(" confirm  ", dim),
            Span::styled("Esc", accent),
            Span::styled(" skip  ", dim),
        ]));

        lines.push(Line::from(""));
        // Bottom separator
        lines.push(Line::from(Span::styled(sep, dim)));

        lines
    }
}
