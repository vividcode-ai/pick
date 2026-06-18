//! Question dialog component for user input
//! Maps to opencode's question tool design

/// Represents a question being asked to the user
pub struct QuestionDialog {
    pub questions: Vec<QuestionState>,
    pub current_index: usize,
    pub custom_input: Option<String>,
    pub custom_mode: bool,
}

pub struct QuestionState {
    pub question: String,
    pub header: String,
    pub options: Vec<QuestionOptionState>,
    pub multiple: bool,
    pub selected: Vec<usize>,
}

pub struct QuestionOptionState {
    pub label: String,
    pub description: String,
}

impl QuestionDialog {
    pub fn new(
        questions: Vec<(String, String, Vec<(String, String)>, bool)>,
    ) -> Self {
        let questions: Vec<QuestionState> = questions
            .into_iter()
            .map(|(q, h, opts, multi)| QuestionState {
                question: q,
                header: h,
                multiple: multi,
                selected: if multi { Vec::new() } else { vec![0] },
                options: opts
                    .into_iter()
                    .map(|(l, d)| QuestionOptionState {
                        label: l,
                        description: d,
                    })
                    .collect(),
            })
            .collect();

        Self {
            questions,
            current_index: 0,
            custom_input: None,
            custom_mode: false,
        }
    }

    pub fn current(&self) -> Option<&QuestionState> {
        self.questions.get(self.current_index)
    }

    pub fn total(&self) -> usize {
        self.questions.len()
    }

    pub fn is_last(&self) -> bool {
        self.current_index + 1 >= self.questions.len()
    }

    pub fn progress_text(&self) -> String {
        format!("Question {}/{}", self.current_index + 1, self.total())
    }

    pub fn render(&self, width: u16) -> Vec<ratatui::text::Line<'static>> {
        use ratatui::prelude::*;
        let width = width as usize;
        let mut lines: Vec<Line> = Vec::new();

        let dim = Style::default().add_modifier(Modifier::DIM);
        let _accent = Style::default().fg(Color::Cyan);
        let selected_style = Style::default().bg(Color::Rgb(50, 80, 120)).fg(Color::White);

        // Top separator
        let sep = "\u{2500}".repeat(width);
        lines.push(Line::from(Span::styled(sep.clone(), dim)));

        // Progress
        lines.push(Line::from(Span::styled(
            format!("  {}", self.progress_text()),
            dim,
        )));

        // Separator
        lines.push(Line::from(Span::styled(sep.clone(), dim)));

        if let Some(q) = self.current() {
            // Header label
            lines.push(Line::from(Span::styled(
                format!("  {}", q.header),
                Style::default().add_modifier(Modifier::BOLD),
            )));

            // Question text
            let wrapped = textwrap::wrap(&q.question, width.saturating_sub(4));
            for w in wrapped {
                lines.push(Line::from(Span::raw(format!("  {}", w))));
            }

            lines.push(Line::from(""));

            if self.custom_mode {
                // Custom input mode
                lines.push(Line::from(Span::styled(
                    "  Type your answer (Enter to submit, Esc to cancel):",
                    dim,
                )));
                let input = self.custom_input.as_deref().unwrap_or("");
                lines.push(Line::from(Span::raw(format!("  > {}", input))));
            } else {
                // Options
                for (i, opt) in q.options.iter().enumerate() {
                    let is_selected = if q.multiple {
                        q.selected.contains(&i)
                    } else {
                        i == q.selected.first().copied().unwrap_or(0)
                    };

                    if is_selected {
                        let marker = if q.multiple { "[✓]" } else { "[✓]" };
                        let text = format!("  {} {} - {}", marker, opt.label, opt.description);
                        lines.push(Line::from(Span::styled(text, selected_style)));
                    } else {
                        let marker = if q.multiple { "[ ]" } else { "[ ]" };
                        let text = format!("  {} {} - {}", marker, opt.label, opt.description);
                        lines.push(Line::from(Span::styled(text, Style::default())));
                    }
                }

                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    if q.multiple {
                        "  \u{2191}\u{2193} navigate  Space toggle  Enter confirm  / custom input  Esc cancel"
                    } else {
                        "  \u{2191}\u{2193} navigate  Enter confirm  / custom input  Esc cancel"
                    },
                    dim,
                )));
            }
        }

        // Bottom separator
        lines.push(Line::from(Span::styled(sep, dim)));

        lines
    }
}
