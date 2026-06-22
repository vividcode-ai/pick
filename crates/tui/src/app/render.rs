//! TUI rendering functions

use std::io::Write;

use crossterm::terminal::size;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::terminal_manager::TerminalManager;
use crate::utils::{visible_width, wrap_text_with_ansi};

use super::types::AppState;
use super::types::TuiApp;
use super::types::format_cwd_for_footer;
use super::types::format_tokens;
#[cfg(windows)]
use super::types::set_windows_terminal_title;

impl TuiApp {
    /// Full render: write chat content to stdout and redraw editor at bottom.
    pub fn render_with_terminal<B>(
        &mut self,
        manager: &mut TerminalManager<B>,
    ) -> Result<(), String>
    where
        B: ratatui::backend::Backend + Write,
        std::io::Error: From<B::Error>,
    {
        let (width, height) = size().map_err(|_| "Failed to get terminal size".to_string())?;

        // Resize reflow: when terminal width changes, invalidate
        // scrollback cache so all entries are re-rendered at new width.
        if self.last_render_width != 0 && self.last_render_width != width {
            self.cached_lines_entry_count = 0;
        }
        self.last_render_width = width;

        if self.state != self.last_render_state {
            manager.invalidate_viewport();
            self.last_render_state.clone_from(&self.state);
        }
        let editor_line_count = self.compute_editor_line_count(width);
        let mut autocomplete_lines = if self.state == AppState::Selecting
            || self.state == AppState::TreeSelecting
            || self.state == AppState::ApiKeyInput
        {
            0_u16
        } else {
            self.editor.autocomplete_line_count() as u16
        };

        let dialog_lines = match self.state {
            AppState::Questioning => {
                if let Some(ref dialog) = self.question_dialog {
                    dialog.render(width).len() as u16
                } else {
                    0
                }
            }
            AppState::UpdatePrompt => {
                if let Some(ref prompt) = self.update_prompt {
                    prompt.render(width).len() as u16
                } else {
                    0
                }
            }
            _ => 0,
        };
        let has_dialog = dialog_lines > 0;

        if has_dialog {
            autocomplete_lines = 0;
        }
        let status_lines = if self.status_text.is_some() {
            1_u16
        } else {
            0_u16
        };
        let has_selection = matches!(
            self.state,
            AppState::Selecting | AppState::TreeSelecting | AppState::ApiKeyInput
        );

        let entry_count = self.chat.entry_count();
        if entry_count > self.cached_lines_entry_count || self.chat.cache_dirty {
            let all_lines = self.chat.render_lines(width as usize, usize::MAX);
            if entry_count > self.cached_lines_entry_count {
                let start = self.cached_lines_committed.min(all_lines.len());
                let new_lines = all_lines[start..].to_vec();
                if !new_lines.is_empty() {
                    manager.insert_history(new_lines);
                }
                self.cached_lines_committed = all_lines.len();
            } else {
                let new_len = all_lines.len();
                if new_len > self.cached_lines_committed {
                    let start = self.cached_lines_committed.min(new_len);
                    let new_lines = all_lines[start..].to_vec();
                    manager.insert_history(new_lines);
                }
                self.cached_lines_committed = new_len;
            }
            self.cached_lines_entry_count = entry_count;
            self.chat.cache_dirty = false;
        }

        let stream_lines = if self.state == AppState::Streaming {
            self.chat.render_active_stream(width as usize)
        } else {
            vec![]
        };
        let stream_chat_len = stream_lines.len() as u16;
        let has_status = status_lines > 0;
        let has_ac = autocomplete_lines > 0;
        if has_ac {
            self.autocomplete_space_lines = autocomplete_lines;
        } else {
            self.autocomplete_space_lines = 0;
        }
        let has_pending = !self.pending_user_messages.is_empty();
        let (pending_lines, pending_overflow) = if has_pending {
            Self::compute_pending_message_lines(&self.pending_user_messages, width as usize)
        } else {
            (0, false)
        };
        let pending_lines = pending_lines as u16;

        let has_todo = !self.todo_items.is_empty()
            && self.todo_items.iter().any(|t| {
                let s = t.get("status").and_then(|v| v.as_str()).unwrap_or("");
                s != "completed" && s != "cancelled"
            });
        let todo_lines: u16 = if has_todo {
            self.render_todo_lines(width as usize).len() as u16
        } else {
            0
        };

        let fixed_non_stream = 1u16
            + editor_line_count
            + 1
            + 2
            + if has_status { 3 } else { 0 }
            + if has_pending {
                pending_lines + (if pending_overflow { 2 } else { 1 })
            } else {
                0
            }
            + if has_todo { todo_lines + 1 } else { 0 }
            + self.autocomplete_space_lines
            + dialog_lines;
        let stream_extra = if stream_chat_len > 0 { 2 } else { 0 };
        let stream_display_len =
            stream_chat_len.min(height.saturating_sub(fixed_non_stream + stream_extra));
        let has_stream = stream_display_len > 0;

        let mut constraints: Vec<Constraint> = Vec::with_capacity(12);
        if has_stream {
            constraints.push(Constraint::Length(1));
            constraints.push(Constraint::Length(stream_display_len));
            constraints.push(Constraint::Length(1));
        }
        if has_status {
            constraints.push(Constraint::Length(1));
            constraints.push(Constraint::Length(1));
            constraints.push(Constraint::Length(1));
        }
        if has_todo {
            for _ in 0..todo_lines {
                constraints.push(Constraint::Length(1));
            }
            constraints.push(Constraint::Length(1));
        }
        if has_pending {
            constraints.push(Constraint::Length(1));
            for _ in 0..pending_lines {
                constraints.push(Constraint::Length(1));
            }
            if pending_overflow {
                constraints.push(Constraint::Length(1));
                constraints.push(Constraint::Length(1));
            } else {
                constraints.push(Constraint::Length(1));
            }
        }
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Length(editor_line_count));
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Length(1));
        if self.autocomplete_space_lines > 0 {
            constraints.push(Constraint::Length(self.autocomplete_space_lines));
        }
        if dialog_lines > 0 {
            constraints.push(Constraint::Length(dialog_lines));
        }

        let content_height: u16 = constraints
            .iter()
            .map(|c| match c {
                Constraint::Length(len) => *len,
                _ => 0,
            })
            .sum();

        let top_border = Line::from(Span::styled(
            "\u{2500}".repeat(width as usize),
            Style::default().add_modifier(Modifier::DIM),
        ));

        manager
            .draw(
                content_height,
                self.autocomplete_space_lines + dialog_lines,
                |frame: &mut crate::custom_terminal::Frame| {
                    let area = frame.area();
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints(constraints.clone())
                        .split(area);

                    let mut i = 0;

                    // Helper: render to chunks[i] with bounds check
                    macro_rules! render_at {
                        ($idx:expr, $widget:expr) => {
                            if $idx < chunks.len() {
                                frame.render_widget_ref(&$widget, chunks[$idx]);
                            }
                        };
                    }

                    if has_stream {
                        render_at!(i, Line::from(""));
                        i += 1;
                        if i < chunks.len() {
                            let overflow =
                                stream_lines.len().saturating_sub(chunks[i].height as usize);
                            let paragraph = Paragraph::new(ratatui::text::Text::from(stream_lines))
                                .scroll((overflow as u16, 0));
                            frame.render_widget_ref(&paragraph, chunks[i]);
                        }
                        i += 1;
                        render_at!(i, Line::from(""));
                        i += 1;
                    }

                    if has_status {
                        render_at!(i, Line::from(""));
                        i += 1;

                        if i < chunks.len()
                            && let Some(ref status) = self.status_text
                        {
                            let frame_idx = self.status_frame % Self::SPINNER_FRAMES.len();
                            let spinner = Self::SPINNER_FRAMES[frame_idx];
                            let display_text = if let Some(start) = self.agent_start_time {
                                let secs = start.elapsed().as_secs();
                                format!(
                                    "{} {} ({} • esc to interrupt)",
                                    spinner,
                                    status,
                                    Self::format_elapsed(secs)
                                )
                            } else {
                                format!("{} {}", spinner, status)
                            };
                            let status_line = Line::from(Span::styled(
                                display_text,
                                Style::default().add_modifier(Modifier::DIM),
                            ));
                            frame.render_widget_ref(&status_line, chunks[i]);
                        }
                        i += 1;

                        render_at!(i, Line::from(""));
                        i += 1;
                    }

                    if has_todo {
                        let todo_lines_rendered = self.render_todo_lines(width as usize);
                        for line in &todo_lines_rendered {
                            render_at!(i, line.clone());
                            i += 1;
                        }
                        render_at!(i, Line::from(""));
                        i += 1;
                    }

                    if has_pending {
                        let user_bg = Style::default().bg(self
                            .chat
                            .colors
                            .user_msg_bg
                            .unwrap_or(Color::Rgb(52, 53, 69)));
                        let content_width = (width as usize).saturating_sub(4).max(1);
                        render_at!(i, Line::from(""));
                        i += 1;

                        let max_visible = 5usize;
                        let max_lines_per_msg = 3usize;
                        for (msg_idx, msg) in self.pending_user_messages.iter().enumerate() {
                            if msg_idx >= max_visible {
                                break;
                            }
                            if i >= chunks.len() {
                                break;
                            }
                            let wrapped = wrap_text_with_ansi(msg, content_width);
                            let display_lines = wrapped.len().min(max_lines_per_msg);
                            for line_idx in 0..display_lines {
                                if i >= chunks.len() {
                                    break;
                                }
                                let line_text = &wrapped[line_idx];
                                let vis = visible_width(line_text);
                                let right_pad = (width as usize).saturating_sub(vis + 2);
                                let padded = format!("  {}{}", line_text, " ".repeat(right_pad));
                                frame.render_widget_ref(
                                    &Line::from(Span::styled(padded, user_bg)),
                                    chunks[i],
                                );
                                i += 1;
                            }
                            if msg_idx + 1 < max_visible {
                                render_at!(i, Line::from(""));
                                i += 1;
                            }
                        }

                        if pending_overflow {
                            let extra = self.pending_user_messages.len() - max_visible;
                            let overflow_line = Line::from(Span::styled(
                                format!("  \u{2026} {} more pending", extra),
                                Style::default().add_modifier(Modifier::DIM),
                            ));
                            render_at!(i, overflow_line);
                            i += 1;
                            render_at!(i, Line::from(""));
                            i += 1;
                        } else {
                            render_at!(i, Line::from(""));
                            i += 1;
                        }
                    }

                    render_at!(i, top_border.clone());
                    i += 1;

                    if i < chunks.len() {
                        match self.state {
                            AppState::Selecting => {
                                let popup = self.build_selection_popup_lines(width);
                                frame.render_widget_ref(
                                    &Paragraph::new(ratatui::text::Text::from(popup)),
                                    chunks[i],
                                );
                            }
                            AppState::TreeSelecting => {
                                let popup = self.render_tree_view_lines(width);
                                frame.render_widget_ref(
                                    &Paragraph::new(ratatui::text::Text::from(popup)),
                                    chunks[i],
                                );
                            }
                            AppState::ApiKeyInput => {
                                frame.render_widget_ref(&Clear, chunks[i]);
                                let popup = self.build_apikey_popup_lines(width);
                                frame.render_widget_ref(
                                    &Paragraph::new(ratatui::text::Text::from(popup)),
                                    chunks[i],
                                );
                                // Set cursor right after "> " on the input line (7th visual line, 0-indexed)
                                let cursor_col = 2u16.min(width.saturating_sub(1));
                                let cursor_row = chunks[i].y + 6;
                                frame.set_cursor_position((cursor_col, cursor_row));
                            }
                            AppState::UpdatePrompt => {
                                // render empty editor area (the dialog is drawn as overlay)
                            }
                            _ => {
                                let editor_max = editor_line_count as usize;
                                let (editor_lines, cursor_row, cursor_col) =
                                    self.editor.render(width as usize, editor_max);
                                frame.render_widget_ref(
                                    &Paragraph::new(ratatui::text::Text::from(editor_lines)),
                                    chunks[i],
                                );
                                if !has_selection {
                                    frame.set_cursor_position((
                                        cursor_col as u16,
                                        chunks[i].y + cursor_row as u16,
                                    ));
                                }
                            }
                        }
                    }
                    i += 1;

                    render_at!(i, top_border.clone());
                    i += 1;

                    if !has_ac && !has_dialog {
                        render_at!(i, self.render_footer_line1(width));
                        if i + 1 < chunks.len() {
                            frame
                                .render_widget_ref(&self.render_footer_line2(width), chunks[i + 1]);
                        }
                    }

                    if self.autocomplete_space_lines > 0 {
                        i += 1;
                    }

                    if dialog_lines > 0 {
                        i += 1;
                    }

                    if has_ac && i >= 2 && i - 2 < chunks.len() {
                        let ac_content = self
                            .editor
                            .render_autocomplete(width as usize, autocomplete_lines as usize);
                        let bottom_sep = &chunks[i - 2];
                        let ac_y = bottom_sep.y + bottom_sep.height;
                        let ac_area = Rect::new(0, ac_y, width, autocomplete_lines);
                        frame.render_widget_ref(
                            &Paragraph::new(ratatui::text::Text::from(ac_content)),
                            ac_area,
                        );
                    }

                    if has_dialog && i >= 4 && i - 4 < chunks.len() {
                        let dialog_content = match self.state {
                            AppState::UpdatePrompt => {
                                self.render_update_prompt_lines(width as usize)
                            }
                            _ => self.render_question_lines(width as usize),
                        };
                        let top_sep = &chunks[i - 4];
                        let overlay_y = top_sep.y;
                        let overlay_area = Rect::new(0, overlay_y, width, dialog_lines);
                        frame.render_widget_ref(
                            &Paragraph::new(ratatui::text::Text::from(dialog_content)),
                            overlay_area,
                        );
                    }
                },
            )
            .map_err(|e| format!("Render error: {}", e))?;

        Ok(())
    }

    /// Build the startup header as a boxed layout with ANSI styling.
    pub fn build_startup_header(&self, width: usize) -> Vec<String> {
        let max_box_inner = (width / 2).saturating_sub(2);

        let title_raw = format!("🤖 Pick v{}", self.version);
        let cwd_display = format_cwd_for_footer(&self.cwd, self.home_dir.as_deref());
        let mut natural = title_raw.chars().count();
        natural = natural.max(format!("directory: {}", cwd_display).chars().count());
        if self.thinking_level != "off" {
            natural = natural.max(
                format!(
                    "model:     {} {}   /model to change",
                    self.model_id, self.thinking_level
                )
                .chars()
                .count(),
            );
        } else {
            natural = natural.max(
                format!("model:     {}   /model to change", self.model_id)
                    .chars()
                    .count(),
            );
        }
        if !self.context_file_names.is_empty() {
            natural = natural.max(
                format!("[Context]  {}", self.context_file_names.join(", "))
                    .chars()
                    .count(),
            );
        }
        if !self.skill_names.is_empty() {
            natural = natural.max(
                format!("[Skills]   {}", self.skill_names.join(", "))
                    .chars()
                    .count(),
            );
        }

        let inner = natural.max(60).min(max_box_inner);
        let content_width = inner - 1;
        let mut lines: Vec<String> = Vec::new();

        let box_line = |content: &str| -> String {
            let vis = visible_width(content);
            let pad = content_width.saturating_sub(vis);
            format!("│ {}{}│", content, "\u{00a0}".repeat(pad))
        };

        let wrap = |text: &str| -> Vec<String> {
            let mut wrapped = Vec::new();
            let mut cur = String::new();
            for word in text.split(' ') {
                let cur_vis = visible_width(&cur);
                let word_vis = visible_width(word);
                if !cur.is_empty() && cur_vis + 1 + word_vis > content_width {
                    wrapped.push(cur.clone());
                    cur = word.to_string();
                } else if cur.is_empty() {
                    cur = word.to_string();
                } else {
                    cur.push(' ');
                    cur.push_str(word);
                }
            }
            if !cur.is_empty() {
                wrapped.push(cur);
            }
            wrapped
        };

        lines.push(format!("╭{}╮", "─".repeat(inner)));

        let title = format!(
            "\x1b[1m🤖 {}\x1b[0m\x1b[2m v{}\x1b[0m",
            self.app_name, self.version
        );
        lines.push(box_line(&title));
        lines.push(box_line(""));

        let model = format!(
            "\x1b[2mmodel:\x1b[0m\u{00a0}\u{00a0}\u{00a0}\u{00a0}\u{00a0}\x1b[1m{}\x1b[0m{}\x1b[2m\u{00a0}\u{00a0}\u{00a0}/model to change\x1b[0m",
            self.model_id,
            if self.thinking_level == "off" {
                String::new()
            } else {
                format!(" {}", self.thinking_level)
            }
        );
        lines.push(box_line(&model));

        let dir = format!("\x1b[2mdirectory:\x1b[0m\u{00a0}{}", cwd_display);
        lines.push(box_line(&dir));
        lines.push(box_line(""));

        let app_title = if self.app_name == "Pick" {
            "Pick"
        } else {
            &self.app_name
        };
        let desc = format!(
            "{} can explain its own features and look up its docs. Ask it how to use or extend {}.",
            app_title, app_title
        );
        for line in wrap(&desc) {
            lines.push(box_line(&format!("\x1b[2m{}\x1b[0m", line)));
        }

        if !self.context_file_names.is_empty() || !self.skill_names.is_empty() {
            lines.push(box_line(""));
        }
        if !self.context_file_names.is_empty() {
            let ctx = format!(
                "\x1b[1m[Context]\x1b[0m\u{00a0}\u{00a0}{}",
                self.context_file_names.join(", ")
            );
            lines.push(box_line(&ctx));
        }
        if !self.skill_names.is_empty() {
            let skl = format!(
                "\x1b[1m[Skills]\x1b[0m\u{00a0}\u{00a0}\u{00a0}{}",
                self.skill_names.join(", ")
            );
            lines.push(box_line(&skl));
        }

        lines.push(format!("╰{}╯", "─".repeat(inner)));
        lines.push(
            "\x1b[2mTip: escape interrupt · ctrl+c/ctrl+d clear/exit · / commands · ! bash · ctrl+o more\x1b[0m"
                .to_string(),
        );

        lines
    }

    /// Add startup header to the chat view (called once on first render)
    pub fn ensure_startup_header(&mut self, width: usize) {
        if self.startup_header_added {
            return;
        }
        self.startup_header_added = true;

        let header_lines = self.build_startup_header(width);
        let combined = header_lines.join("\n");
        self.chat.add_system_message(&combined);
    }

    /// Reset and re-show the startup header (for /new).
    pub fn show_startup_header(&mut self, width: usize) {
        self.startup_header_added = false;
        self.ensure_startup_header(width);
    }

    /// Render footer line 1: mode indicator left, CWD with git branch and session name right.
    pub fn render_footer_line1(&self, width: u16) -> Line<'static> {
        let mode_style = Style::default()
            .fg(Color::Rgb(255, 165, 0))
            .add_modifier(Modifier::BOLD);
        let mode_text = if self.agent_mode == "plan" {
            "\u{23f5}\u{23f5} Plan mode"
        } else {
            "\u{23f5}\u{23f5} Build mode"
        };
        let dim = Style::default().add_modifier(Modifier::DIM);

        let mut right_target = self.model_id.width();
        if self.thinking_level != "off" {
            right_target += self.thinking_level.width() + 3;
        }

        let mut pwd = format_cwd_for_footer(&self.cwd, self.home_dir.as_deref());
        if let Some(ref branch) = self.git_branch {
            pwd = format!("{} \u{2022} {}", pwd, branch);
        }

        if pwd.width() > right_target
            && let Some(sep_pos) = pwd.rfind(" \u{2022} ")
        {
            let suffix = &pwd[sep_pos..];
            let suffix_w = suffix.width();
            let max_path = right_target.saturating_sub(suffix_w);
            let path_part = &pwd[..sep_pos];
            if path_part.width() > max_path && max_path >= 6 {
                let mut tail = String::new();
                let mut w = 4;
                for c in path_part.chars().rev() {
                    let cw = c.to_string().width();
                    if w + cw > max_path {
                        break;
                    }
                    tail.insert(0, c);
                    w += cw;
                }
                pwd = format!("... {}{}", tail, suffix);
            }
        }
        let pwd_w = pwd.width();
        if pwd_w < right_target {
            let pad = " ".repeat(right_target - pwd_w);
            pwd = format!("{}{}", pad, pwd);
        }

        let mode_w = mode_text.width();
        let pwd_w2 = pwd.width();
        let padding = (width as usize).saturating_sub(mode_w + pwd_w2);
        Line::from(vec![
            Span::styled(mode_text.to_string(), mode_style),
            Span::styled(" ".repeat(padding), dim),
            Span::styled(pwd, dim),
        ])
    }

    /// Render footer line 2: context capacity left + model name right.
    pub fn render_footer_line2(&self, width: u16) -> Line<'static> {
        let dim = Style::default().add_modifier(Modifier::DIM);
        let auto_indicator = if self.auto_compact { " (auto)" } else { "" };
        let left_side = match self.context_percent {
            Some(pct) => format!(
                "{:.1}%/{}{}",
                pct,
                format_tokens(self.context_window),
                auto_indicator
            ),
            None => format!("?/{}{}", format_tokens(self.context_window), auto_indicator),
        };
        let mut right_side = self.model_id.clone();
        if self.thinking_level != "off" {
            right_side = format!("{} \u{2022} {}", right_side, self.thinking_level);
        }
        let padding = (width as usize).saturating_sub(left_side.width() + right_side.width());
        if padding > 0 {
            Line::from(vec![
                Span::styled(left_side, dim),
                Span::styled(" ".repeat(padding), dim),
                Span::styled(right_side, dim),
            ])
        } else {
            Line::from(vec![
                Span::styled(left_side, dim),
                Span::styled("  ".to_string(), dim),
                Span::styled(right_side, dim),
            ])
        }
    }

    /// Render the todo list viewport (between status bar and pending messages)
    pub fn render_todo_lines(&self, width: usize) -> Vec<Line<'static>> {
        use ratatui::prelude::*;
        if self.todo_items.is_empty() {
            return vec![];
        }

        let dim = Style::default().add_modifier(Modifier::DIM);
        let accent = Style::default().fg(Color::Cyan);
        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(Span::styled(
            " Todo Plan ",
            Style::default().add_modifier(Modifier::BOLD),
        )));

        let active_items: Vec<&serde_json::Value> = self
            .todo_items
            .iter()
            .filter(|t| {
                let s = t.get("status").and_then(|v| v.as_str()).unwrap_or("");
                s != "completed" && s != "cancelled"
            })
            .collect();
        let scroll = self
            .todo_scroll_offset
            .min(active_items.len().saturating_sub(1));
        let visible_items: Vec<&serde_json::Value> =
            active_items.iter().skip(scroll).take(5).copied().collect();

        for item in &visible_items {
            let content = item.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("");

            let (icon, base_style): (&str, Style) = match status {
                "in_progress" => (" \u{25b6}", accent),
                "pending" => (" \u{25cb}", dim),
                _ => (" \u{25cf}", dim),
            };

            let line_style = base_style;

            let priority = item.get("priority").and_then(|v| v.as_str()).unwrap_or("");
            let (priority_label, priority_style) = match priority {
                "high" => (" \u{9ad8}", Color::Red),
                "medium" => (" \u{4e2d}", Color::Yellow),
                "low" => (" \u{4f4e}", Color::DarkGray),
                _ => ("", Color::Reset),
            };

            let truncated: String = content.chars().take(width.saturating_sub(8)).collect();
            lines.push(Line::from(vec![
                Span::styled(icon, line_style),
                Span::raw(" "),
                Span::styled(truncated, line_style),
                if !priority_label.is_empty() {
                    Span::styled(priority_label, line_style.fg(priority_style))
                } else {
                    Span::raw("")
                },
            ]));
        }

        let remaining = active_items.len().saturating_sub(scroll + 5);
        if remaining > 0 {
            lines.push(Line::from(Span::styled(
                format!("  \u{2026} {} more", remaining),
                dim,
            )));
        }

        lines
    }

    /// Render the question dialog for the current questioning state
    pub fn render_question_lines(&self, width: usize) -> Vec<Line<'static>> {
        match self.question_dialog {
            Some(ref dialog) => dialog.render(width as u16),
            None => vec![],
        }
    }

    pub fn render_update_prompt_lines(&self, width: usize) -> Vec<Line<'static>> {
        match self.update_prompt {
            Some(ref prompt) => prompt.render(width as u16),
            None => vec![],
        }
    }

    /// Compute editor line count for layout
    pub fn compute_editor_line_count(&self, width: u16) -> u16 {
        if self.state == AppState::Selecting {
            let item_count = self.selection.as_ref().map(|s| s.items.len()).unwrap_or(0);
            let has_desc = self
                .selection
                .as_ref()
                .and_then(|s| s.selected())
                .and_then(|i| i.description.as_ref())
                .is_some();
            let has_search = self.selection.as_ref().is_some_and(|s| s.has_search());
            // base: N items + title(1) + position(1) + empty(1) + hint(1) = N + 4
            // + desc: + desc(1) + empty(1) = N + 6
            // + search bar: +1 = N + 5 or N + 7
            let base: u16 = if has_desc { 6 } else { 4 };
            let search_extra: u16 = if has_search { 1 } else { 0 };
            let reserved = base + search_extra;
            let visible = std::cmp::min(item_count, 10) as u16;
            (reserved + visible).clamp(5, 14)
        } else if self.state == AppState::TreeSelecting {
            let count = self
                .tree_view
                .as_ref()
                .map(|tv| tv.visible_count())
                .unwrap_or(0);
            let visible = std::cmp::min(count, 12) as u16;
            (visible + 5).clamp(5, 18)
        } else if self.state == AppState::ApiKeyInput {
            11_u16
        } else if self.state == AppState::UpdatePrompt {
            1_u16
        } else if self.editor.buffer.is_empty() {
            1_u16
        } else {
            std::cmp::max(1, self.editor.visual_line_count(width as usize).min(5)) as u16
        }
    }

    /// Compute the number of visual lines occupied by pending user messages
    pub fn compute_pending_message_lines(
        pending: &std::collections::VecDeque<String>,
        width: usize,
    ) -> (usize, bool) {
        if pending.is_empty() {
            return (0, false);
        }
        let max_visible = 5usize;
        let max_lines_per = 3usize;
        let content_width = width.saturating_sub(4).max(1);
        let mut total = 0usize;
        let count = pending.len().min(max_visible);

        for (idx, msg) in pending.iter().enumerate() {
            if idx >= count {
                break;
            }
            let wrapped = wrap_text_with_ansi(msg, content_width);
            total += wrapped.len().min(max_lines_per);
            if idx + 1 < count {
                total += 1;
            }
        }

        let overflow = pending.len() > max_visible;
        (total, overflow)
    }

    /// Update shared chat render cache
    pub fn build_selection_popup_lines(&self, width: u16) -> Vec<Line<'static>> {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let cyan = Style::default().fg(Color::Cyan);
        let dim = Style::default().add_modifier(Modifier::DIM);
        if let Some(ref sel) = self.selection {
            let mut result: Vec<Line<'static>> = Vec::new();

            // Search / filter bar (only show when user is actively searching)
            if sel.has_search() {
                let search_display = sel.search_query.clone();
                result.push(Line::from(Span::styled(
                    format!("> {}", search_display),
                    dim,
                )));
            }

            result.push(Line::from(Span::styled(sel.title.clone(), bold)));
            let label_max = std::cmp::min(32, width.saturating_sub(4) as usize);
            let start = sel.page_start();
            let end = sel.page_end();
            if sel.items.is_empty() {
                result.push(Line::from(Span::styled("  No matches", dim)));
            }
            for i in start..end {
                let item = &sel.items[i];
                let selected = i == sel.selected_index;
                if selected {
                    result.push(Line::from(vec![
                        Span::styled("\u{2192}".to_string(), cyan),
                        Span::raw(" ".to_string()),
                        Span::styled(item.label.clone(), cyan),
                    ]));
                } else {
                    let truncated = if item.label.len() > label_max {
                        let max_bytes = label_max.saturating_sub(1);
                        let mut end = max_bytes;
                        while !item.label.is_char_boundary(end) {
                            end -= 1;
                        }
                        format!("{}...", &item.label[..end])
                    } else {
                        format!(
                            "{}{}",
                            item.label,
                            " ".repeat(label_max.saturating_sub(item.label.len()))
                        )
                    };
                    result.push(Line::from(Span::raw(truncated)));
                }
            }
            let total = sel.items.len();
            if total > 0 {
                let current_pos = sel.selected_index + 1;
                result.push(Line::from(Span::styled(
                    format!("  ({}/{})", current_pos, total),
                    dim,
                )));
            }
            result.push(Line::from(""));
            if let Some(desc) = sel.selected().and_then(|i| i.description.as_ref()) {
                let desc_trimmed: String = desc
                    .chars()
                    .take(width.saturating_sub(4) as usize)
                    .collect();
                result.push(Line::from(Span::styled(format!(" {}", desc_trimmed), dim)));
                result.push(Line::from(""));
            }
            result.push(Line::from(Span::styled(
                "Enter/Space to select \u{00B7} Esc to cancel".to_string(),
                dim,
            )));
            result
        } else {
            vec![]
        }
    }

    pub fn build_apikey_popup_lines(&self, _width: u16) -> Vec<Line<'static>> {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let dim = Style::default().add_modifier(Modifier::DIM);
        let provider = self.api_key_provider.as_deref().unwrap_or("provider");
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(Span::styled(
            format!("Connect to {}", provider),
            bold,
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(""));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::raw("  Enter your API key:")));
        lines.push(Line::from(""));
        let input_display = if self.api_key_input.is_empty() {
            Line::from(Span::styled("  <type your API key>".to_string(), dim))
        } else {
            let masked_len = self.api_key_input.len().saturating_sub(4);
            let masked = "\u{2022}".repeat(masked_len);
            let last_four = &self.api_key_input[self.api_key_input.len().saturating_sub(4)..];
            Line::from(Span::raw(format!("  {}{}", masked, last_four)))
        };
        lines.push(Line::from(
            vec![Span::styled(">".to_string(), dim), Span::raw(" ")]
                .into_iter()
                .chain(input_display.spans)
                .collect::<Vec<_>>(),
        ));
        lines.push(Line::from(""));
        lines.push(Line::from(""));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Enter to confirm \u{00B7} Esc to cancel".to_string(),
            dim,
        )));
        lines
    }

    /// Braille spinner frames
    const SPINNER_FRAMES: [&'static str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    fn format_elapsed(secs: u64) -> String {
        if secs >= 60 {
            format!("{}m {:02}s", secs / 60, secs % 60)
        } else {
            format!("{}s", secs)
        }
    }

    /// Update terminal title to match current app state.
    pub fn update_terminal_title(&self) {
        let indicator = match self.state {
            AppState::Streaming => {
                if (self.status_frame / 5).is_multiple_of(2) {
                    "☀️"
                } else {
                    "  "
                }
            }
            _ => "✅",
        };
        let title = match self.session_name.as_deref() {
            Some(name) => format!("{indicator} Pick - {name} - {}", self.folder),
            None => format!("{indicator} Pick - {}", self.folder),
        };

        #[cfg(windows)]
        set_windows_terminal_title(&title);
        #[cfg(not(windows))]
        {
            let _ = write!(std::io::stdout(), "\x1b]0;{title}\x07");
            let _ = std::io::stdout().flush();
        }
    }

    /// Set session name and update terminal title
    pub fn set_session_name(&mut self, name: String) {
        self.session_name = Some(name);
        self.update_terminal_title();
    }

    /// Extract Kitty image IDs from rendered lines.
    /// Parses `i=<id>` from Kitty graphics protocol escape sequences.
    pub fn set_colors(&mut self, colors: crate::components::theme::TuiColors) {
        self.chat.colors = colors;
    }

    /// Show an error message
    pub fn show_error(&mut self, err: &str) {
        self.chat.add_error(err);
    }

    /// Show usage info
    pub fn show_usage(&mut self, input: u64, output: u64) {
        let duration_secs = self
            .agent_start_time
            .as_ref()
            .map(|t| t.elapsed().as_secs());
        self.chat.add_usage(input, output, duration_secs);
        self.total_input = self.total_input.saturating_add(input);
        self.total_output = self.total_output.saturating_add(output);
    }

    /// Update cache stats for footer display
    pub fn set_cache_stats(&mut self, cache_read: u64, cache_write: u64) {
        self.total_cache_read = self.total_cache_read.saturating_add(cache_read);
        self.total_cache_write = self.total_cache_write.saturating_add(cache_write);
    }

    /// Update context window info
    pub fn set_context_info(&mut self, percent: Option<f64>, window: u64) {
        self.context_percent = percent;
        self.context_window = window;
    }

    /// Update git branch
    pub fn set_git_branch(&mut self, branch: Option<String>) {
        self.git_branch = branch;
    }

    // --- Agent streaming ---

    /// Append text to the ongoing assistant message
    pub fn stream_content(&mut self, text: &str) {
        self.chat.stream_assistant_content(text);
        if self.state != AppState::Streaming {
            self.state = AppState::Streaming;
            self.update_terminal_title();
        }
    }

    /// Append content after the current assistant content
    pub fn append_content(&mut self, text: &str) {
        self.chat.append_assistant_content(text);
        if self.state != AppState::Streaming {
            self.state = AppState::Streaming;
            self.update_terminal_title();
        }
    }

    /// Add a pending tool execution entry to the chat
    pub fn add_tool_execution(
        &mut self,
        tool_call_id: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) {
        self.chat.add_tool_execution(tool_call_id, tool_name, args);
    }

    /// Update a tool execution entry with result
    pub fn update_tool_execution(&mut self, tool_call_id: &str, output: &str, is_error: bool) {
        self.chat
            .update_tool_execution(tool_call_id, output, is_error);
    }

    /// Append partial output to a running tool execution
    pub fn update_tool_execution_output(&mut self, tool_call_id: &str, partial: &str) {
        self.chat
            .update_tool_execution_output(tool_call_id, partial);
    }

    /// Finalize the current turn (assistant done).
    pub fn finalize_turn(&mut self) {
        self.chat.mark_turn_end();
        if self.state != AppState::Selecting
            && self.state != AppState::TreeSelecting
            && self.state != AppState::ApiKeyInput
        {
            self.state = AppState::Input;
            self.update_terminal_title();
        }
        self.paste_burst.clear_after_explicit_paste();
    }

    /// Set the status text shown between chat and editor.
    pub fn set_status(&mut self, status: Option<&str>) {
        self.status_text = status.map(|s| s.to_string());
        if status.is_some() {
            self.status_frame = 0;
        }
    }

    /// Set the status text without starting the spinner animation.
    pub fn set_goal_status(&mut self, status: Option<&str>) {
        self.status_text = status.map(|s| s.to_string());
    }

    /// Advance the spinner animation frame by one.
    pub fn advance_spinner(&mut self) {
        self.status_frame = self.status_frame.wrapping_add(1);
    }

    pub fn start_agent_timer(&mut self) {
        self.agent_start_time = Some(std::time::Instant::now());
    }

    pub fn stop_agent_timer(&mut self) {
        self.agent_start_time = None;
    }
}
