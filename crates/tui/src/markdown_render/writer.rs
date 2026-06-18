use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use pulldown_cmark::Alignment;
use pulldown_cmark::CodeBlockKind;
use pulldown_cmark::Event;
use pulldown_cmark::Tag;
use pulldown_cmark::TagEnd;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use unicode_width::UnicodeWidthStr;

use crate::markdown_render::styles::MarkdownStyles;

pub(crate) struct Writer {
    pub text: Text<'static>,
    pub styles: MarkdownStyles,
    indent_stack: Vec<IndentContext>,
    list_indices: Vec<Option<u64>>,
    inline_styles: Vec<Style>,
    link: Option<LinkState>,
    wrap_width: Option<usize>,
    table_state: Option<TableState>,
    code_block_buffer: String,
    code_block_lang: Option<String>,
    in_code_block: bool,
    in_table: bool,
    pending_spans: Vec<Span<'static>>,
    cwd: Option<PathBuf>,
}

#[derive(Clone, Debug)]
struct IndentContext {
    prefix: String,
    hanging: bool,
}

#[derive(Clone, Debug)]
struct LinkState {
    dest: String,
    show_dest: bool,
    local: bool,
}

#[derive(Clone, Debug)]
struct TableState {
    headers: Vec<String>,
    alignments: Vec<Alignment>,
    rows: Vec<Vec<String>>,
    current_row: Vec<String>,
    current_cell: String,
    col_index: usize,
    is_header: bool,
}

impl Writer {
    pub(crate) fn new(width: Option<usize>, cwd: Option<PathBuf>) -> Self {
        Self {
            text: Text::default(),
            styles: MarkdownStyles::default(),
            indent_stack: Vec::new(),
            list_indices: Vec::new(),
            inline_styles: Vec::new(),
            link: None,
            wrap_width: width,
            table_state: None,
            code_block_buffer: String::new(),
            code_block_lang: None,
            in_code_block: false,
            in_table: false,
            pending_spans: Vec::new(),
            cwd,
        }
    }

    pub(crate) fn handle_event(&mut self, event: &Event<'_>) {
        match event {
            Event::Start(tag) => self.handle_start_tag(tag),
            Event::End(tag_end) => self.handle_end_tag(tag_end),
            Event::Text(text) => self.handle_text(text),
            Event::Code(text) => self.handle_code(text),
            Event::Html(html) | Event::InlineHtml(html) => {
                self.flush_pending();
                self.emit_line_inner(vec![self.make_span(html.to_string())]);
            }
            Event::SoftBreak => {
                self.pending_spans.push(Span::raw(" "));
            }
            Event::HardBreak => {
                self.flush_pending();
                self.emit_blank_line();
            }
            Event::Rule => {
                self.flush_pending();
                let width = self.wrap_width.unwrap_or(80);
                let rule = "─".repeat(width.min(80));
                self.emit_line_inner(vec![Span::styled(rule, self.styles.hr)]);
            }
            _ => {}
        }
    }

    fn handle_start_tag(&mut self, tag: &Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                self.flush_pending();
            }
            Tag::Heading { level, .. } => {
                let level = (*level as usize).min(6).max(1);
                let heading_style = self.styles.heading[level - 1];
                self.push_style(heading_style);
            }
            Tag::BlockQuote(_kind) => {
                self.flush_pending();
                self.push_indent("> ", true);
                self.push_style(self.styles.quote);
            }
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(lang) if !lang.is_empty() => Some(lang.to_string()),
                    _ => None,
                };
                self.code_block_lang = lang;
                self.code_block_buffer.clear();
                self.in_code_block = true;
            }
            Tag::List(list_start) => {
                self.flush_pending();
                self.list_indices.push(*list_start);
                let depth = self.list_indices.len().saturating_sub(1);
                self.push_indent(&"  ".repeat(depth), false);
            }
            Tag::Item => {
                self.flush_pending();
                let indent = self.current_indent().to_string();
                let bullet =
                    if let Some(idx) = self.list_indices.last_mut().and_then(|i| i.as_mut()) {
                        let s = format!("{}.  ", *idx);
                        *idx += 1;
                        s
                    } else {
                        "• ".to_string()
                    };
                self.push_indent(&format!("{}{}", indent, bullet), false);
            }
            Tag::Table(alignments) => {
                self.in_table = true;
                self.table_state = Some(TableState {
                    headers: Vec::new(),
                    alignments: alignments.clone(),
                    rows: Vec::new(),
                    current_row: Vec::new(),
                    current_cell: String::new(),
                    col_index: 0,
                    is_header: true,
                });
            }
            Tag::TableHead => {
                if let Some(ref mut ts) = self.table_state {
                    ts.is_header = true;
                }
            }
            Tag::TableRow => {
                if let Some(ref mut ts) = self.table_state {
                    ts.current_row.clear();
                    ts.col_index = 0;
                }
            }
            Tag::TableCell => {
                if let Some(ref mut ts) = self.table_state {
                    ts.current_cell.clear();
                }
            }
            Tag::Emphasis => {
                self.push_style(self.styles.italic);
            }
            Tag::Strong => {
                self.push_style(self.styles.bold);
            }
            Tag::Strikethrough => {
                self.push_style(self.styles.strikethrough);
            }
            Tag::Link { dest_url, .. } => {
                let dest = dest_url.to_string();
                let show_dest = !dest.is_empty();
                let is_local = dest.starts_with("file://")
                    || dest.starts_with('/')
                    || dest.starts_with("~/")
                    || dest.starts_with("./")
                    || dest.starts_with("../");
                self.link = Some(LinkState {
                    dest: dest.clone(),
                    show_dest,
                    local: is_local,
                });
            }
            _ => {}
        }
    }

    fn handle_end_tag(&mut self, tag: &TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                self.flush_pending();
            }
            TagEnd::Heading(_) => {
                self.flush_pending();
                self.pop_style();
            }
            TagEnd::BlockQuote(_kind) => {
                self.flush_pending();
                self.pop_indent();
                self.pop_style();
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                let code = std::mem::take(&mut self.code_block_buffer);
                let lang = self.code_block_lang.take();
                let highlighted = self.render_highlighted_code(&code, lang.as_deref());
                for line in highlighted {
                    self.emit_line_inner(line.spans);
                }
                self.emit_blank_line();
            }
            TagEnd::List(_) => {
                self.flush_pending();
                self.list_indices.pop();
                self.pop_indent();
            }
            TagEnd::Item => {
                self.flush_pending();
                self.pop_indent();
            }
            TagEnd::Table => {
                self.in_table = false;
                self.finalize_table();
            }
            TagEnd::TableHead => {
                if let Some(ref mut ts) = self.table_state {
                    ts.headers = std::mem::take(&mut ts.current_row);
                    ts.is_header = false;
                }
            }
            TagEnd::TableRow => {
                if let Some(ref mut ts) = self.table_state {
                    let row = std::mem::take(&mut ts.current_row);
                    ts.rows.push(row);
                }
            }
            TagEnd::TableCell => {
                if let Some(ref mut ts) = self.table_state {
                    let cell = std::mem::take(&mut ts.current_cell);
                    ts.current_row.push(cell);
                }
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.pop_style();
            }
            TagEnd::Link => {
                self.flush_pending();
                if let Some(link) = self.link.take() {
                    if link.show_dest && !link.local {
                        let display_dest = if link.dest.len() > 60 {
                            format!("…{}", &link.dest[link.dest.len().saturating_sub(59)..])
                        } else {
                            link.dest.clone()
                        };
                        self.emit_line_inner(vec![Span::styled(
                            format!(" ({})", display_dest),
                            self.styles.link_url,
                        )]);
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_text(&mut self, text: &str) {
        if self.table_state.is_some() {
            if let Some(ref mut ts) = self.table_state {
                ts.current_cell.push_str(text);
            }
            return;
        }

        if self.code_block_lang.is_some() {
            self.code_block_buffer.push_str(text);
            return;
        }

        self.pending_spans.push(self.make_span(text.to_string()));
    }

    fn handle_code(&mut self, text: &str) {
        self.pending_spans
            .push(Span::styled(format!("`{}`", text), self.styles.code));
    }

    fn current_indent(&self) -> &str {
        self.indent_stack
            .last()
            .map(|ic| ic.prefix.as_str())
            .unwrap_or("")
    }

    fn push_indent(&mut self, prefix: &str, hanging: bool) {
        self.indent_stack.push(IndentContext {
            prefix: prefix.to_string(),
            hanging,
        });
    }

    fn pop_indent(&mut self) {
        self.indent_stack.pop();
    }

    fn push_style(&mut self, style: Style) {
        let base = self.inline_styles.last().copied().unwrap_or_default();
        self.inline_styles.push(base.patch(style));
    }

    fn pop_style(&mut self) {
        self.inline_styles.pop();
    }

    fn current_style(&self) -> Style {
        self.inline_styles.last().copied().unwrap_or_default()
    }

    fn make_span(&self, content: impl Into<Cow<'static, str>>) -> Span<'static> {
        let mut span = Span::styled(content.into(), self.current_style());
        if let Some(ref link) = self.link {
            if link.show_dest {
                span = span.style(self.styles.link);
            }
        }
        span
    }

    fn push_span(&mut self, span: Span<'static>) {
        self.pending_spans.push(span);
    }

    fn flush_pending(&mut self) {
        if !self.pending_spans.is_empty() {
            let mut spans = std::mem::take(&mut self.pending_spans);
            if let Some(first) = spans.first_mut() {
                let content = first.content.as_ref().to_string();
                if let Some(rest) = content.strip_prefix("• ") {
                    first.content = std::borrow::Cow::Owned(if rest.is_empty() {
                        String::new()
                    } else {
                        rest.to_string()
                    });
                }
            }
            let filtered: Vec<_> = spans
                .into_iter()
                .filter(|s| !s.content.as_ref().is_empty())
                .collect();
            if !filtered.is_empty() {
                self.emit_line_inner(filtered);
            }
        }
    }

    fn emit_line(&mut self, spans: Vec<Span<'static>>) {
        self.flush_pending();
        self.emit_line_inner(spans);
    }

    fn emit_line_inner(&mut self, spans: Vec<Span<'static>>) {
        let indent = self.current_indent();
        let should_wrap = !self.in_code_block && !self.in_table && self.wrap_width.is_some();

        if should_wrap {
            let width = self.wrap_width.unwrap();
            let indent_w = indent.width();
            let avail = width.saturating_sub(indent_w).max(1);
            let opts = crate::wrapping::RtOptions::new(avail)
                .initial_indent(Line::from(Span::raw(indent.to_string())))
                .subsequent_indent(Line::from(Span::raw(" ".repeat(indent_w))));
            let wrapped = crate::wrapping::adaptive_wrap_lines([Line::from(spans)], opts);
            self.text.lines.extend(wrapped);
        } else {
            let mut full_spans = Vec::new();
            if !indent.is_empty() {
                full_spans.push(Span::raw(indent.to_string()));
            }
            full_spans.extend(spans);
            self.text.lines.push(Line::from(full_spans));
        }
    }

    fn emit_blank_line(&mut self) {
        self.text.lines.push(Line::from(""));
    }

    fn render_highlighted_code(&self, code: &str, lang: Option<&str>) -> Vec<Line<'static>> {
        if let Some(ref lang) = lang {
            let lines = crate::syntax_highlight::highlight_code_to_lines(code, lang);
            if !lines.is_empty() && !(lines.len() == 1 && lines[0].spans.is_empty()) {
                return lines;
            }
        }
        code.lines()
            .map(|l| Line::from(Span::styled(l.to_string(), self.styles.code_block)))
            .collect()
    }

    pub(crate) fn has_table_state(&self) -> bool {
        self.table_state.is_some()
    }

    pub(crate) fn finalize_table(&mut self) {
        let ts = self.table_state.take();
        let Some(ts) = ts else { return };

        if ts.headers.is_empty() && ts.rows.is_empty() {
            return;
        }

        let width = self.wrap_width.unwrap_or(80).saturating_sub(4);
        let col_count = ts
            .headers
            .len()
            .max(ts.rows.iter().map(|r| r.len()).max().unwrap_or(0));
        if col_count == 0 {
            return;
        }

        let mut col_widths = vec![0usize; col_count];
        for (i, h) in ts.headers.iter().enumerate() {
            col_widths[i] = col_widths[i].max(h.chars().count());
        }
        for row in &ts.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < col_count {
                    col_widths[i] = col_widths[i].max(cell.chars().count());
                }
            }
        }

        let total_min = col_widths.len() * 3 + 1;
        let available = width.max(total_min);
        for w in &mut col_widths {
            let max_col =
                available.saturating_sub(total_min.saturating_sub(*w + 2)) / col_count.max(1);
            *w = (*w).min(max_col.max(3));
        }

        let separator = format!(
            "┌{}┐",
            col_widths
                .iter()
                .map(|w| "─".repeat(*w + 2))
                .collect::<Vec<_>>()
                .join("┬")
        );
        self.emit_line(vec![Span::styled(separator, self.styles.code_block_border)]);

        if !ts.headers.is_empty() {
            let header_row = format!(
                "│{}│",
                ts.headers
                    .iter()
                    .enumerate()
                    .map(|(i, h)| {
                        let w = col_widths.get(i).copied().unwrap_or(3);
                        format!(" {:<w$} ", h, w = w)
                    })
                    .collect::<Vec<_>>()
                    .join("│")
            );
            self.emit_line(vec![Span::styled(header_row, self.styles.code_block)]);

            let sep = format!(
                "├{}┤",
                col_widths
                    .iter()
                    .map(|w| "─".repeat(*w + 2))
                    .collect::<Vec<_>>()
                    .join("┼")
            );
            self.emit_line(vec![Span::styled(sep, self.styles.code_block_border)]);
        }

        for row in &ts.rows {
            let row_str = format!(
                "│{}│",
                (0..col_count)
                    .map(|i| {
                        let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
                        let w = col_widths[i];
                        format!(" {:<w$} ", cell, w = w)
                    })
                    .collect::<Vec<_>>()
                    .join("│")
            );
            self.emit_line(vec![Span::styled(row_str, self.styles.code_block)]);
        }

        let bottom = format!(
            "└{}┘",
            col_widths
                .iter()
                .map(|w| "─".repeat(*w + 2))
                .collect::<Vec<_>>()
                .join("┴")
        );
        self.emit_line(vec![Span::styled(bottom, self.styles.code_block_border)]);
    }
}

#[allow(dead_code)]
pub(crate) fn parse_local_link_target(dest: &str, cwd: Option<&Path>) -> (String, String) {
    let dest = dest.trim();
    let (path_str, location) = if let Some(pos) = dest.rfind(|c| c == '#' || c == ':') {
        let (p, loc) = dest.split_at(pos);
        (p.to_string(), loc.to_string())
    } else {
        (dest.to_string(), String::new())
    };

    let display_path = if let Some(ref cwd) = cwd {
        if let Some(rel) = pathdiff::diff_paths(&path_str, cwd) {
            rel.to_string_lossy().to_string()
        } else {
            path_str.clone()
        }
    } else {
        path_str.clone()
    };

    (display_path, location)
}
