//! Inserts finalized history rows into terminal scrollback.
//!
//! Uses escape-sequence operations directly (bypassing ratatui buffer) to write
//! finalized chat history into the terminal scrollback above the viewport.

use std::io;
use std::io::Write;

use crossterm::cursor::MoveDown;
use crossterm::cursor::MoveTo;
use crossterm::cursor::MoveToColumn;
use crossterm::queue;
use crossterm::style::Color as CColor;
use crossterm::style::Colors;
use crossterm::style::Print;
use crossterm::style::SetAttribute;
use crossterm::style::SetBackgroundColor;
use crossterm::style::SetColors;
use crossterm::style::SetForegroundColor;
use crossterm::terminal::Clear;
use crossterm::terminal::ClearType;
use ratatui::layout::Size;
use ratatui::prelude::Backend;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::text::Line;
use ratatui::text::Span;

use crate::custom_terminal::Terminal;
use crate::custom_terminal::to_ct_color;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_line;
use crate::wrapping::line_contains_url_like;
use crate::wrapping::line_has_mixed_url_and_non_url_tokens;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryLineWrapPolicy {
    PreWrap,
    Terminal,
}

/// Insert `lines` above the viewport using the terminal's backend writer.
/// Returns the number of wrapped rows actually inserted.
pub fn insert_history_lines<B>(
    terminal: &mut Terminal<B>,
    lines: Vec<Line>,
) -> io::Result<u16>
where
    B: Backend + Write,
    io::Error: From<B::Error>,
{
    insert_history_lines_with_wrap_policy(terminal, lines, HistoryLineWrapPolicy::PreWrap)
}

/// Insert `lines` above the viewport using the terminal's backend writer.
/// Returns the number of wrapped rows actually inserted.
pub fn insert_history_lines_with_wrap_policy<B>(
    terminal: &mut Terminal<B>,
    lines: Vec<Line>,
    wrap_policy: HistoryLineWrapPolicy,
) -> io::Result<u16>
where
    B: Backend + Write,
    io::Error: From<B::Error>,
{
    let screen_size = terminal.backend().size().unwrap_or(Size::new(0, 0));

    let mut area = terminal.viewport_area;
    let mut should_update_area = false;
    let last_cursor_pos = terminal.last_known_cursor_pos;
    let writer = terminal.backend_mut();

    // Pre-wrap lines for terminal scrollback.
    // URL-only-ish lines keep intact (no hard newlines inserted).
    // Mixed lines are adaptively wrapped.
    let wrap_width = area.width.max(1) as usize;
    let mut wrapped = Vec::new();
    let mut wrapped_rows = 0usize;

    for line in &lines {
        let line_wrapped = match wrap_policy {
            HistoryLineWrapPolicy::Terminal => vec![line.clone()],
            HistoryLineWrapPolicy::PreWrap
                if line_contains_url_like(line) && !line_has_mixed_url_and_non_url_tokens(line) =>
            {
                vec![line.clone()]
            }
            HistoryLineWrapPolicy::PreWrap => adaptive_wrap_line(
                line,
                RtOptions::new(wrap_width).subsequent_indent(leading_whitespace_prefix(line)),
            ),
        };
        wrapped_rows += line_wrapped
            .iter()
            .map(|wrapped_line| wrapped_line.width().max(1).div_ceil(wrap_width))
            .sum::<usize>();
        wrapped.extend(line_wrapped);
    }
    let wrapped_lines = wrapped_rows as u16;

    let cursor_top = if area.bottom() < screen_size.height {
        // If viewport is not at bottom, scroll down to make room.
        let scroll_amount = wrapped_lines.min(screen_size.height - area.bottom());

        // Set scroll region and scroll down
        write!(writer, "\x1b[{};{}r", area.top() + 1, screen_size.height)?;
        queue!(writer, MoveTo(0, area.top()))?;
        for _ in 0..scroll_amount {
            queue!(writer, Print("\x1bM"))?;
        }
        write!(writer, "\x1b[r")?; // Reset scroll region

        let cursor_top = area.top().saturating_sub(1);
        area.y += scroll_amount;
        should_update_area = true;
        cursor_top
    } else {
        area.top().saturating_sub(1)
    };

    // Set scroll region from top of screen to top of viewport
    write!(writer, "\x1b[1;{}r", area.top())?;
    queue!(writer, MoveTo(0, cursor_top))?;

    for line in &wrapped {
        queue!(writer, Print("\r\n"))?;
        write_history_line(writer, line, wrap_width)?;
    }

    // Reset scroll region
    write!(writer, "\x1b[r")?;

    // Restore cursor position
    queue!(writer, MoveTo(last_cursor_pos.x, last_cursor_pos.y))?;

    if should_update_area {
        terminal.set_viewport_area(area);
    }
    if wrapped_lines > 0 {
        terminal.note_history_rows_inserted(wrapped_lines);
    }

    Ok(wrapped_lines)
}

fn leading_whitespace_prefix(line: &Line<'_>) -> Line<'static> {
    let mut spans = Vec::new();
    for span in &line.spans {
        let prefix_end = span
            .content
            .char_indices()
            .find_map(|(idx, ch)| (!ch.is_whitespace()).then_some(idx))
            .unwrap_or(span.content.len());
        if prefix_end > 0 {
            spans.push(Span::styled(
                span.content[..prefix_end].to_string(),
                span.style,
            ));
        }
        if prefix_end < span.content.len() {
            break;
        }
    }
    Line::from(spans).style(line.style)
}

fn write_history_line<W: Write>(writer: &mut W, line: &Line, wrap_width: usize) -> io::Result<()> {
    let physical_rows = line.width().max(1).div_ceil(wrap_width) as u16;
    if physical_rows > 1 {
        // Clear continuation rows for wide lines
        for _ in 1..physical_rows {
            queue!(writer, MoveDown(1), MoveToColumn(0))?;
            queue!(writer, Clear(ClearType::UntilNewLine))?;
        }
        // Move back to start for rendering
        let up = physical_rows.saturating_sub(1);
        if up > 0 {
            queue!(writer, crossterm::cursor::MoveUp(up))?;
        }
    }

    // Emit line-level style as the initial state
    let line_fg = line.style.fg;
    let line_bg = line.style.bg;
    if line_fg.is_some() || line_bg.is_some() {
        queue!(
            writer,
            SetColors(Colors::new(
                to_ct_color(line_fg.unwrap_or(Color::Reset)),
                to_ct_color(line_bg.unwrap_or(Color::Reset)),
            ))
        )?;
    }
    // Clear to ensure we start fresh
    queue!(writer, Clear(ClearType::UntilNewLine))?;

    let mut fg = line_fg;
    let mut bg = line_bg;
    let mut last_modifier = Modifier::empty();

    for span in &line.spans {
        // Merge line-level style into each span so that line-level
        // fg/bg/modifier propagate correctly through history lines.
        let span_style = span.style.patch(line.style);

        // Modifier computation: start from empty, apply both
        // add_modifier and sub_modifier
        let mut modifier = Modifier::empty();
        modifier.insert(span_style.add_modifier);
        modifier.remove(span_style.sub_modifier);

        let span_fg = to_ct_color(span_style.fg.unwrap_or(Color::Reset));
        let span_bg = to_ct_color(span_style.bg.unwrap_or(Color::Reset));

        if span_style.fg != fg || span_style.bg != bg {
            queue!(writer, SetColors(Colors::new(span_fg, span_bg)))?;
            fg = span_style.fg;
            bg = span_style.bg;
        }

        if modifier != last_modifier {
            let diff = ModifierDiff {
                from: last_modifier,
                to: modifier,
            };
            diff.queue(writer)?;
            last_modifier = modifier;
        }

        queue!(writer, Print(span.content.as_ref()))?;
    }

    // Reset styles (including modifiers so DIM/BOLD/ITALIC don't leak across lines)
    queue!(
        writer,
        SetAttribute(crossterm::style::Attribute::Reset),
        SetForegroundColor(CColor::Reset),
        SetBackgroundColor(CColor::Reset)
    )?;

    Ok(())
}

struct ModifierDiff {
    from: Modifier,
    to: Modifier,
}

impl ModifierDiff {
    fn queue<W: io::Write>(self, w: &mut W) -> io::Result<()> {
        use crossterm::style::Attribute as CAttribute;

        let removed = self.from - self.to;
        if removed.contains(Modifier::REVERSED) {
            queue!(w, SetAttribute(CAttribute::NoReverse))?;
        }
        if removed.contains(Modifier::BOLD) {
            queue!(w, SetAttribute(CAttribute::NormalIntensity))?;
            if self.to.contains(Modifier::DIM) {
                queue!(w, SetAttribute(CAttribute::Dim))?;
            }
        }
        if removed.contains(Modifier::ITALIC) {
            queue!(w, SetAttribute(CAttribute::NoItalic))?;
        }
        if removed.contains(Modifier::UNDERLINED) {
            queue!(w, SetAttribute(CAttribute::NoUnderline))?;
        }
        if removed.contains(Modifier::DIM) {
            queue!(w, SetAttribute(CAttribute::NormalIntensity))?;
        }
        if removed.contains(Modifier::CROSSED_OUT) {
            queue!(w, SetAttribute(CAttribute::NotCrossedOut))?;
        }
        if removed.contains(Modifier::SLOW_BLINK) || removed.contains(Modifier::RAPID_BLINK) {
            queue!(w, SetAttribute(CAttribute::NoBlink))?;
        }

        let added = self.to - self.from;
        if added.contains(Modifier::REVERSED) {
            queue!(w, SetAttribute(CAttribute::Reverse))?;
        }
        if added.contains(Modifier::BOLD) {
            queue!(w, SetAttribute(CAttribute::Bold))?;
        }
        if added.contains(Modifier::DIM) {
            queue!(w, SetAttribute(CAttribute::Dim))?;
        }
        if added.contains(Modifier::ITALIC) {
            queue!(w, SetAttribute(CAttribute::Italic))?;
        }
        if added.contains(Modifier::UNDERLINED) {
            queue!(w, SetAttribute(CAttribute::Underlined))?;
        }
        if added.contains(Modifier::CROSSED_OUT) {
            queue!(w, SetAttribute(CAttribute::CrossedOut))?;
        }
        if added.contains(Modifier::SLOW_BLINK) {
            queue!(w, SetAttribute(CAttribute::SlowBlink))?;
        }
        if added.contains(Modifier::RAPID_BLINK) {
            queue!(w, SetAttribute(CAttribute::RapidBlink))?;
        }

        Ok(())
    }
}
