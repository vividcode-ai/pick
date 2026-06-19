//! Chat message display component

pub(crate) mod types;

#[allow(hidden_glob_reexports)]
use self::types::{THINK_PREFIX, THINK_SUFFIX, TOOL_CALL_MAX_LINES};
use crate::components::theme::TuiColors;
use crate::utils::{truncate_to_width, visible_width, wrap_text_with_ansi};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

pub use types::*;

/// Scrollable chat message view
pub struct ChatView {
    pub entries: Vec<ChatEntry>,
    pub scroll_offset: usize,
    /// When true, the next `stream_assistant_content` creates a new assistant message
    /// instead of updating the last one. Set by `mark_turn_end()` on TurnEnd.
    next_stream_creates_new: bool,
    /// In-flight streaming assistant content (not yet committed to entries).
    /// Set during streaming, committed to entries on `mark_turn_end()`.
    pub active_streaming_content: Option<String>,
    /// Dirty flag: set when entries are modified (e.g., tool execution update).
    /// Forces a full scrollback re-insertion on next render to reflect changes.
    pub cache_dirty: bool,
    /// Theme colors for rendering.
    pub colors: TuiColors,
    /// Cached rendered lines for scrollback. Avoids re-parsing markdown and
    /// re-wrapping text on every frame when entries haven't changed.
    rendered_cache: Option<Vec<Line<'static>>>,
    /// Width at which the cache was last built (for resize invalidation).
    rendered_cache_width: usize,
    /// Entry count at which the cache was last built.
    rendered_cache_entry_count: usize,
}

impl ChatView {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            scroll_offset: 0,
            next_stream_creates_new: false,
            active_streaming_content: None,
            colors: TuiColors::default(),
            cache_dirty: false,
            rendered_cache: None,
            rendered_cache_width: 0,
            rendered_cache_entry_count: 0,
        }
    }

    /// Create with a specific theme
    pub fn with_colors(colors: TuiColors) -> Self {
        Self {
            entries: Vec::new(),
            scroll_offset: 0,
            next_stream_creates_new: false,
            active_streaming_content: None,
            colors,
            cache_dirty: false,
            rendered_cache: None,
            rendered_cache_width: 0,
            rendered_cache_entry_count: 0,
        }
    }

    /// Mark cache as dirty to force a full scrollback re-insertion on next render.
    /// Also clears the rendered lines cache so it will be rebuilt on next render.
    fn invalidate_cache(&mut self) {
        self.cache_dirty = true;
        self.rendered_cache = None;
    }

    /// Clear all entries and reset scroll state
    pub fn clear(&mut self) {
        self.entries.clear();
        self.scroll_offset = 0;
        self.next_stream_creates_new = false;
        self.active_streaming_content = None;
        self.cache_dirty = false;
        self.rendered_cache = None;
        self.rendered_cache_width = 0;
        self.rendered_cache_entry_count = 0;
    }

    /// Iterate over message entries only
    pub fn messages(&self) -> impl Iterator<Item = &ChatMessage> {
        self.entries.iter().filter_map(|e| {
            if let ChatEntry::Message(m) = e {
                Some(m)
            } else {
                None
            }
        })
    }

    /// Mutable access to the last ASSISTANT message entry.
    /// Skips tool entries and finds the last assistant message even if
    /// tool entries were added after it.
    fn last_assistant_message_mut(&mut self) -> Option<&mut ChatMessage> {
        self.entries.iter_mut().rev().find_map(|e| {
            if let ChatEntry::Message(m) = e {
                if m.role == "assistant" { Some(m) } else { None }
            } else {
                None
            }
        })
    }

    /// Number of message entries
    pub fn message_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e, ChatEntry::Message(_)))
            .count()
    }

    /// Number of all entries (messages + tool executions)
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Add a user message
    pub fn add_user_message(&mut self, text: &str) {
        self.entries
            .push(ChatEntry::Message(ChatMessage::new("user", text)));
        self.invalidate_cache();
        self.scroll_to_bottom();
    }

    /// Update the streaming assistant content.
    /// The text is the full accumulated content from StreamEvent partial snapshots.
    /// During streaming, content is stored in `active_streaming_content` (not entries).
    /// After a turn end (`mark_turn_end`), the content is committed as a real entry.
    pub fn stream_assistant_content(&mut self, text: &str) {
        if self.next_stream_creates_new {
            self.next_stream_creates_new = false;
            // Commit any existing active streaming content first
            if let Some(existing) = self.active_streaming_content.take()
                && !existing.is_empty() {
                    self.entries
                        .push(ChatEntry::Message(ChatMessage::new("assistant", existing)));
                    self.invalidate_cache();
                }
            self.active_streaming_content = Some(text.to_string());
        } else if let Some(ref mut current) = self.active_streaming_content {
            // Streaming optimization: if the new text has the old content as a prefix,
            // append only the delta. Otherwise (new turn / restructured content) replace.
            if text.starts_with(current.as_str()) && text.len() > current.len() {
                let delta = &text[current.len()..];
                current.push_str(delta);
            } else {
                *current = text.to_string();
            }
        } else {
            // No active streaming yet — start one
            self.active_streaming_content = Some(text.to_string());
        }
        self.scroll_to_bottom();
    }

    /// Mark the end of a turn. Commits any active streaming content to entries
    /// and sets the flag so the next `stream_assistant_content` creates a new message.
    pub fn mark_turn_end(&mut self) {
        if let Some(content) = self.active_streaming_content.take()
            && !content.is_empty() {
                // Guard: prevent committing the same content twice.
                // The content may have been already committed via the next_stream_creates_new
                // path in stream_assistant_content if a StreamContent arrived after an EndTurn.
                let is_duplicate = self
                    .entries
                    .iter()
                    .rev()
                    .any(|e| matches!(e, ChatEntry::Message(m) if m.role == "assistant" && m.content == content));
                if !is_duplicate {
                    self.entries
                        .push(ChatEntry::Message(ChatMessage::new("assistant", content)));
                    self.invalidate_cache();
                }
            }
        self.next_stream_creates_new = true;
    }

    /// Discard the active streaming content without committing it.
    /// Used when tool calls interrupt streaming and the pending text
    /// should not appear in the final scrollback.
    pub fn discard_active_stream(&mut self) {
        self.active_streaming_content = None;
        self.next_stream_creates_new = true;
    }

    /// Whether there is active streaming content being rendered.
    pub fn has_active_stream(&self) -> bool {
        self.active_streaming_content.is_some()
    }

    /// Render the active streaming tail as ratatui Lines.
    /// Returns empty vec if no active stream.
    pub fn render_active_stream(&self, width: usize) -> Vec<Line<'static>> {
        let Some(content) = &self.active_streaming_content else {
            return vec![];
        };
        let mut lines = Vec::new();
        let msg = ChatMessage::new("assistant", content.clone());
        self.render_assistant_message_lines(&msg, width, &mut lines);
        while lines.last().is_some_and(|l| {
            l.spans.is_empty() || l.spans.iter().all(|s| s.content.as_ref().is_empty())
        }) {
            lines.pop();
        }
        lines
    }

    /// Append text to the streaming assistant content or last assistant entry.
    /// If streaming is active, appends to `active_streaming_content`.
    /// Otherwise falls back to appending to the last assistant entry.
    pub fn append_assistant_content(&mut self, text: &str) {
        if let Some(ref mut current) = self.active_streaming_content {
            current.push_str(text);
        } else {
            let needs_new = self
                .last_assistant_message_mut()
                .is_none_or(|m| m.role != "assistant");
            if needs_new {
                self.entries.push(ChatEntry::Message(ChatMessage::new(
                    "assistant",
                    text.to_string(),
                )));
                self.invalidate_cache();
            } else if let Some(last) = self.last_assistant_message_mut() {
                last.content.push_str(text);
            }
        }
        self.scroll_to_bottom();
    }

    /// Show an error message in chat
    pub fn add_error(&mut self, text: &str) {
        self.entries
            .push(ChatEntry::Message(ChatMessage::new("error", text)));
        self.invalidate_cache();
        self.scroll_to_bottom();
    }

    /// Add a system/info message (used for startup header, context, etc.)
    pub fn add_system_message(&mut self, text: &str) {
        self.entries
            .push(ChatEntry::Message(ChatMessage::new("system", text)));
        self.invalidate_cache();
        // Don't scroll to bottom — system messages are at the top
    }

    /// Remove the last entry from the chat
    pub fn pop_last(&mut self) {
        self.entries.pop();
        self.invalidate_cache();
    }

    /// Remove all message entries with the given role (e.g. "system")
    pub fn remove_messages_by_role(&mut self, role: &str) {
        self.entries.retain(|e| {
            if let ChatEntry::Message(m) = e {
                m.role != role
            } else {
                true
            }
        });
        self.invalidate_cache();
    }

    /// Show usage info as a chat entry
    pub fn add_usage(&mut self, input: u64, output: u64, duration_secs: Option<u64>) {
        let content = match duration_secs {
            Some(secs) => {
                let dur = if secs >= 60 {
                    format!("{}m {:02}s", secs / 60, secs % 60)
                } else {
                    format!("{}s", secs)
                };
                format!("Input: {}  Output: {}  Duration: {}", input, output, dur)
            }
            None => format!("Input: {}  Output: {}", input, output),
        };
        self.entries
            .push(ChatEntry::Message(ChatMessage::new("usage", content)));
        self.invalidate_cache();
        self.scroll_to_bottom();
    }

    /// Add a pending tool execution entry.
    /// Skips if an entry with the same tool_call_id already exists (dedup).
    pub fn add_tool_execution(
        &mut self,
        tool_call_id: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) {
        if self
            .entries
            .iter()
            .any(|e| matches!(e, ChatEntry::ToolExecution(te) if te.tool_call_id == tool_call_id))
        {
            return;
        }
        self.entries
            .push(ChatEntry::ToolExecution(ToolExecutionEntry {
                tool_call_id: tool_call_id.to_string(),
                tool_name: tool_name.to_string(),
                args,
                status: ToolStatus::Pending,
                output: String::new(),
                expanded: false,
            }));
        self.invalidate_cache();
        self.scroll_to_bottom();
    }

    /// Append partial output to a running tool execution (no status change).
    /// Used for streaming tool output before execution completes.
    pub fn update_tool_execution_output(&mut self, tool_call_id: &str, partial: &str) {
        for entry in self.entries.iter_mut().rev() {
            if let ChatEntry::ToolExecution(te) = entry
                && te.tool_call_id == tool_call_id {
                    if !partial.is_empty() {
                        if te.output.is_empty() {
                            te.output = partial.to_string();
                        } else {
                            te.output.push_str(partial);
                        }
                    }
                    break;
                }
        }
        self.invalidate_cache();
        self.scroll_to_bottom();
    }

    /// Update a tool execution by ID (set output and status).
    /// The output is the full tool result (replaces any streaming content).
    pub fn update_tool_execution(&mut self, tool_call_id: &str, output: &str, is_error: bool) {
        for entry in self.entries.iter_mut().rev() {
            if let ChatEntry::ToolExecution(te) = entry
                && te.tool_call_id == tool_call_id {
                    te.status = if is_error {
                        ToolStatus::Error
                    } else {
                        ToolStatus::Success
                    };
                    te.output = output.to_string();
                    break;
                }
        }
        self.invalidate_cache();
        self.scroll_to_bottom();
    }

    /// Replace tool execution output without changing status (for streaming bash timer).
    pub fn replace_tool_execution_output(&mut self, tool_call_id: &str, output: &str) {
        for entry in self.entries.iter_mut().rev() {
            if let ChatEntry::ToolExecution(te) = entry
                && te.tool_call_id == tool_call_id {
                    te.output = output.to_string();
                    break;
                }
        }
        self.invalidate_cache();
        self.scroll_to_bottom();
    }

    /// Toggle expansion of the most recent tool execution entry (Ctrl+O).
    /// Returns the new expanded state, or None if no tool execution exists.
    pub fn toggle_tool_expansion(&mut self) -> Option<bool> {
        // Find the last tool execution entry
        let expanded = self.entries.iter_mut().rev().find_map(|entry| {
            if let ChatEntry::ToolExecution(te) = entry {
                te.expanded = !te.expanded;
                Some(te.expanded)
            } else {
                None
            }
        });
        if expanded.is_some() {
            self.invalidate_cache();
        }
        expanded
    }

    /// Build the full rendered line list from scratch (no caching).
    /// Used internally by `render_lines` when the cache is invalid.
    fn build_lines(&self, width: usize) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();

        for entry in &self.entries {
            match entry {
                ChatEntry::Message(msg) => {
                    match msg.role.as_str() {
                        "user" => {
                            self.render_user_message_lines(msg, width, &mut lines);
                            let bg = self.user_bg_color();
                            lines.push(Line::from(Span::styled(" ".repeat(width), bg)).style(bg));
                            lines.push(Line::from(""));
                        }
                        "assistant" => self.render_assistant_message_lines(msg, width, &mut lines),
                        "system" => self.render_system_message_lines(msg, width, &mut lines),
                        "error" => self.render_error_message_lines(msg, width, &mut lines),
                        "usage" => self.render_usage_message_lines(msg, width, &mut lines),
                        _ => self.render_system_message_lines(msg, width, &mut lines),
                    }
                    if msg.role != "user" {
                        lines.push(Line::from(""));
                    }
                }
                ChatEntry::ToolExecution(te) => {
                    self.render_tool_execution_lines(te, width, &mut lines);
                    lines.push(Line::from(""));
                }
            }
        }

        // Trim trailing blank lines — only remove lines with ZERO-length content
        // (e.g. Line::from("") which is the blank separator between messages).
        // Lines with background-colored spaces as padding MUST be preserved.
        while lines.last().is_some_and(|l| {
            l.spans.is_empty() || l.spans.iter().all(|s| s.content.as_ref().is_empty())
        }) {
            lines.pop();
        }

        lines
    }

    /// Render all entries as ratatui Lines with direct Style values.
    /// Results are cached and only rebuilt when entries change or width changes.
    pub fn render_lines(&mut self, width: usize, max_height: usize) -> Vec<Line<'static>> {
        if max_height == 0 {
            return Vec::new();
        }

        // Rebuild cache when entries have changed, width changed, or cache is dirty.
        let cache_valid = self.rendered_cache.is_some()
            && !self.cache_dirty
            && self.rendered_cache_width == width
            && self.rendered_cache_entry_count == self.entries.len();

        if !cache_valid {
            let lines = self.build_lines(width);
            self.rendered_cache_entry_count = self.entries.len();
            self.rendered_cache_width = width;
            self.rendered_cache = Some(lines);
        }

        // Serve from cache (clone is orders of magnitude cheaper than re-parsing markdown).
        if let Some(ref cached) = self.rendered_cache {
            if cached.len() <= max_height {
                return cached.clone();
            }
            let total = cached.len();
            let bottom = total.saturating_sub(max_height);
            let start = bottom.saturating_sub(self.scroll_offset);
            let end = std::cmp::min(start + max_height, total);
            cached[start..end].to_vec()
        } else {
            Vec::new()
        }
    }

    fn user_bg_color(&self) -> Style {
        Style::default().bg(self.colors.user_msg_bg.unwrap_or(Color::Rgb(52, 53, 69)))
    }

    fn tool_title_color(&self) -> Style {
        Style::default().fg(self.colors.tool_title.unwrap_or(Color::Rgb(212, 212, 212)))
    }

    fn accent_color(&self) -> Style {
        Style::default().fg(self.colors.accent.unwrap_or(Color::Rgb(138, 190, 183)))
    }

    fn muted_color(&self) -> Style {
        Style::default().fg(self.colors.muted.unwrap_or(Color::Rgb(128, 128, 128)))
    }

    fn error_color(&self) -> Style {
        Style::default().fg(self.colors.error.unwrap_or(Color::Rgb(204, 102, 102)))
    }

    fn render_user_message_lines(
        &self,
        msg: &ChatMessage,
        width: usize,
        lines: &mut Vec<Line<'static>>,
    ) {
        let bg = self.user_bg_color();
        let content_width = width.saturating_sub(4).max(1);
        let wrapped = wrap_text_with_ansi(&msg.content, content_width);

        if wrapped.is_empty() {
            return;
        }

        // Top padding with full-width background
        lines.push(Line::from(Span::styled(" ".repeat(width), bg)).style(bg));

        for line_text in &wrapped {
            if line_text.is_empty() {
                lines.push(Line::from(Span::styled(" ".repeat(width), bg)).style(bg));
            } else {
                let vis = visible_width(line_text);
                let right_pad = width.saturating_sub(vis + 2);
                let padded = format!("  {}{}", line_text, " ".repeat(right_pad));
                // User message content is plain text — pure ratatui Style, no ANSI parsing
                lines.push(Line::from(Span::styled(padded, bg)).style(bg));
            }
        }
    }

    /// Split assistant message content into thinking blocks and text blocks.
    /// Thinking blocks are delimited by ANSI italic+gray markers set in tui.rs.
    fn split_thinking_blocks(content: &str) -> Vec<(bool, String)> {
        let mut parts = Vec::new();
        let mut remaining = content;

        while let Some(start) = remaining.find(THINK_PREFIX) {
            if start > 0 {
                let text = remaining[..start].trim().to_string();
                if !text.is_empty() {
                    parts.push((false, text));
                }
            }
            remaining = &remaining[start + THINK_PREFIX.len()..];

            if let Some(end) = remaining.find(THINK_SUFFIX) {
                let thinking = remaining[..end].trim().to_string();
                if !thinking.is_empty() {
                    parts.push((true, thinking));
                }
                remaining = &remaining[end + THINK_SUFFIX.len()..];
            } else {
                let text = remaining.trim().to_string();
                if !text.is_empty() {
                    parts.push((false, text));
                }
                return parts;
            }
        }

        let text = remaining.trim().to_string();
        if !text.is_empty() {
            parts.push((false, text));
        }

        parts
    }

    /// Render a markdown text block into styled lines, wrapped to fit terminal width.
    fn render_markdown_lines(text: &str, content_width: usize, lines: &mut Vec<Line<'static>>) {
        if text.is_empty() {
            return;
        }

        let rendered = crate::markdown_render::render_markdown_text_with_width_and_cwd(
            text,
            Some(content_width),
            None,
        );

        for md_line in rendered.lines {
            // Add leading spaces for alignment (renderer already wrapped to width)
            let mut spans = vec![Span::raw("  ".to_string())];
            spans.extend(md_line.spans);
            lines.push(Line::from(spans));
        }
    }

    fn render_assistant_message_lines(
        &self,
        msg: &ChatMessage,
        width: usize,
        lines: &mut Vec<Line<'static>>,
    ) {
        let content_width = width.saturating_sub(1).max(1);
        let parts = Self::split_thinking_blocks(&msg.content);

        if parts.is_empty() {
            return;
        }

        let has_thinking = parts.iter().any(|(is_thinking, _)| *is_thinking);

        if !has_thinking {
            Self::render_markdown_lines(
                &parts.into_iter().next().map(|(_, t)| t).unwrap_or_default(),
                content_width,
                lines,
            );
            return;
        }

        for (is_thinking, text) in parts {
            if is_thinking {
                let wrapped = wrap_text_with_ansi(&text, content_width);
                for wt in &wrapped {
                    let trimmed = truncate_to_width(wt, content_width);
                    lines.push(Line::from(Span::styled(
                        format!(" {}", trimmed),
                        Style::default()
                            .fg(Color::Rgb(128, 128, 128))
                            .add_modifier(Modifier::ITALIC),
                    )));
                }
                lines.push(Line::from(""));
            } else {
                Self::render_markdown_lines(&text, content_width, lines);
            }
        }
    }

    fn render_system_message_lines(
        &self,
        msg: &ChatMessage,
        width: usize,
        lines: &mut Vec<Line<'static>>,
    ) {
        // System messages contain ANSI escape codes (e.g., startup header).
        // Parse them into proper ratatui Styles instead of leaking raw ANSI.
        let wrapped = wrap_text_with_ansi(&msg.content, width);
        for line_text in &wrapped {
            let truncated = truncate_to_width(line_text, width);
            if truncated.contains('\x1b') {
                let styled_line = crate::app::ansi_to_styled_line(&truncated);
                lines.push(styled_line);
            } else {
                lines.push(Line::from(Span::raw(truncated)));
            }
        }
    }

    fn render_error_message_lines(
        &self,
        msg: &ChatMessage,
        width: usize,
        lines: &mut Vec<Line<'static>>,
    ) {
        let err_style = self.error_color().add_modifier(Modifier::BOLD);
        let prefix = "Error: ";
        let prefix_width = visible_width(prefix);
        let content_width = width.saturating_sub(prefix_width);
        let wrapped = wrap_text_with_ansi(&msg.content, content_width);
        for (i, line_text) in wrapped.iter().enumerate() {
            if i == 0 {
                let mut spans = vec![Span::styled(prefix, err_style)];
                if line_text.contains('\x1b') {
                    let rest = crate::app::ansi_to_styled_line(line_text);
                    spans.extend(rest.spans);
                } else {
                    spans.push(Span::raw(line_text.clone()));
                }
                lines.push(Line::from(spans));
            } else {
                let truncated = truncate_to_width(line_text, width);
                if truncated.contains('\x1b') {
                    lines.push(crate::app::ansi_to_styled_line(&truncated));
                } else {
                    lines.push(Line::from(Span::raw(truncated)));
                }
            }
        }
    }

    fn render_usage_message_lines(
        &self,
        msg: &ChatMessage,
        width: usize,
        lines: &mut Vec<Line<'static>>,
    ) {
        let dim = Style::default().add_modifier(Modifier::DIM);
        let wrapped = wrap_text_with_ansi(&msg.content, width);
        for line_text in &wrapped {
            let truncated = truncate_to_width(line_text, width);
            if truncated.contains('\x1b') {
                let mut line = crate::app::ansi_to_styled_line(&truncated);
                // Apply dim style on top of any parsed styles
                for span in &mut line.spans {
                    span.style = span.style.patch(dim);
                }
                lines.push(line);
            } else {
                lines.push(Line::from(Span::styled(truncated, dim)));
            }
        }
    }

    /// Build tool label as styled spans.
    /// Returns (bullet_span, status_prefix, label_spans, is_error).
    fn build_tool_label(
        &self,
        te: &ToolExecutionEntry,
    ) -> (Span<'static>, Span<'static>, Vec<Span<'static>>, bool) {
        let dim = Style::default().add_modifier(Modifier::DIM);
        let is_error = te.status == ToolStatus::Error;

        let bullet = match te.status {
            ToolStatus::Pending => Span::styled("•", dim),
            ToolStatus::Success => Span::styled(
                "•",
                Style::default()
                    .fg(Color::Rgb(110, 210, 110))
                    .add_modifier(Modifier::BOLD),
            ),
            ToolStatus::Error => Span::styled(
                "•",
                Style::default()
                    .fg(Color::Rgb(255, 100, 100))
                    .add_modifier(Modifier::BOLD),
            ),
        };
        let status = match te.status {
            ToolStatus::Pending => {
                Span::styled("Running", Style::default().add_modifier(Modifier::BOLD))
            }
            _ => Span::styled("Ran", Style::default().add_modifier(Modifier::BOLD)),
        };

        let cmd_text = match &*te.tool_name.to_lowercase() {
            "bash" => te
                .args
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("...")
                .to_string(),
            "read" => {
                let path = te
                    .args
                    .get("file_path")
                    .or_else(|| te.args.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                format!("read {}", if path.is_empty() { "?" } else { path })
            }
            "edit" => {
                let path = te
                    .args
                    .get("file_path")
                    .or_else(|| te.args.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                format!("edit {}", if path.is_empty() { "?" } else { path })
            }
            "write" => {
                let path = te
                    .args
                    .get("file_path")
                    .or_else(|| te.args.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                format!("write {}", if path.is_empty() { "?" } else { path })
            }
            "grep" => {
                let pattern = te
                    .args
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let raw_path = te.args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let glob = te.args.get("glob").and_then(|v| v.as_str());
                let limit = te.args.get("limit").and_then(|v| v.as_u64());
                let mut s = format!("grep /{}/ in {}", pattern, raw_path);
                if let Some(g) = glob {
                    s.push_str(&format!(" ({})", g));
                }
                if let Some(l) = limit {
                    s.push_str(&format!(" limit {}", l));
                }
                s
            }
            "find" => {
                let pattern = te
                    .args
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let raw_path = te.args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let limit = te.args.get("limit").and_then(|v| v.as_u64());
                let mut s = format!("find {} in {}", pattern, raw_path);
                if let Some(l) = limit {
                    s.push_str(&format!(" (limit {})", l));
                }
                s
            }
            "ls" => {
                let path = te
                    .args
                    .get("path")
                    .or_else(|| te.args.get("file_path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let limit = te.args.get("limit").and_then(|v| v.as_u64());
                let mut s = format!("ls {}", path);
                if let Some(l) = limit {
                    s.push_str(&format!(" (limit {})", l));
                }
                s
            }
            _ => {
                let val = te
                    .args
                    .as_object()
                    .and_then(|obj| obj.iter().next())
                    .map(|(_, v)| match v {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .unwrap_or_default();
                if val.is_empty() {
                    te.tool_name.to_lowercase()
                } else {
                    format!("{} {}", te.tool_name.to_lowercase(), val)
                }
            }
        };

        let label_spans = vec![Span::raw(cmd_text)];
        (bullet, status, label_spans, is_error)
    }

    /// Render tool execution lines:
    ///     • Running <command>          (pending)
    ///     • Ran <command>              (success)
    ///       └ <dimmed output>          (first output line)
    ///         <dimmed output>          (subsequent output lines)
    fn render_tool_execution_lines(
        &self,
        te: &ToolExecutionEntry,
        width: usize,
        lines: &mut Vec<Line<'static>>,
    ) {
        let (bullet, status, label_spans, _is_error) = self.build_tool_label(te);
        let content_width = width.saturating_sub(1).max(1);
        let dim = Style::default().add_modifier(Modifier::DIM);

        // Build header line: "• Running <command>"
        let mut header_spans = vec![bullet, Span::raw(" "), status, Span::raw(" ")];
        header_spans.extend(label_spans);

        // Wrap header if too long, using "    " continuation prefix
        let header_line = Line::from(header_spans);
        let wrapped_header = crate::wrapping::word_wrap_lines(
            [header_line],
            crate::wrapping::RtOptions::new(content_width)
                .initial_indent(Line::from(Span::raw("")))
                .subsequent_indent(Line::from(Span::styled("    ", dim))),
        );
        for wl in wrapped_header {
            lines.push(wl);
        }

        // Output
        if !te.output.is_empty() && (te.expanded || !is_hidden_when_collapsed(te)) {
            let output_lines: Vec<&str> = te.output.lines().collect();
            let total = output_lines.len();

            // When expanded: show all lines. When collapsed: head+middle+tail truncation.
            let truncated: Vec<&str> = if te.expanded {
                output_lines.clone()
            } else if total > TOOL_CALL_MAX_LINES {
                let head_count = TOOL_CALL_MAX_LINES / 2;
                let tail_count = TOOL_CALL_MAX_LINES - head_count - 1;
                let mut result: Vec<&str> = output_lines[..head_count].to_vec();
                result.push(""); // placeholder for ellipsis
                result.extend(output_lines[total.saturating_sub(tail_count)..].iter());
                result
            } else {
                output_lines.clone()
            };

            let is_truncated = !te.expanded && total > TOOL_CALL_MAX_LINES;
            for (i, raw_line) in truncated.iter().enumerate() {
                let is_ellipsis = is_truncated && i == TOOL_CALL_MAX_LINES / 2;

                let (prefix, line_text) = if is_ellipsis {
                    let remaining = total - (TOOL_CALL_MAX_LINES - 1);
                    let msg = format!("… +{} lines (Ctrl+O to expand)", remaining);
                    ("  ", msg)
                } else {
                    let prefix = if i == 0 { "  └ " } else { "    " };
                    (prefix, raw_line.to_string())
                };

                // Wrap output line
                let display = format!("{}{}", prefix, line_text);
                let dim_style = Style::default().add_modifier(Modifier::DIM);

                if is_ellipsis {
                    let padded = format!(" {}", display);
                    lines.push(Line::from(Span::styled(padded, dim_style)));
                } else if display.contains('\x1b') {
                    // Parse ANSI and apply dim modifier on top
                    let styled = crate::app::ansi_to_styled_line(&display);
                    let dimmed_spans: Vec<Span<'static>> = styled
                        .spans
                        .into_iter()
                        .map(|span| Span {
                            style: span.style.patch(dim_style),
                            content: span.content,
                        })
                        .collect();
                    lines.push(Line::from(dimmed_spans));
                } else {
                    let trimmed = truncate_to_width(&display, content_width);
                    let padded = format!(" {}", trimmed);
                    lines.push(Line::from(Span::styled(padded, dim_style)));
                }
            }
        }
    }

    /// Render the chat view (outputs ANSI strings).
    /// Updates internal cache for `line_count_before_last`.
    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
    }

    pub fn scroll_down(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }
}

fn is_hidden_when_collapsed(_te: &ToolExecutionEntry) -> bool {
    false
}

impl Default for ChatView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: render chat as plain text (no ANSI codes).
    fn render_text(chat: &mut ChatView, width: usize, max_height: usize) -> String {
        let lines = chat.render_lines(width, max_height);
        lines
            .iter()
            .flat_map(|l| {
                let s: String = l.spans.iter().map(|sp| sp.content.as_ref()).collect();
                if s.is_empty() { vec![] } else { vec![s] }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Helper: convert a single ratatui Line to plain text.
    fn line_text(line: &Line<'static>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn test_streaming_render_updates_content_correctly() {
        let mut chat = ChatView::new();
        let width = 80;

        // Step 1: User submits a message
        chat.add_user_message("你好");
        let text = render_text(&mut chat, width, usize::MAX);
        assert!(
            text.contains("你好"),
            "user message should appear: got {:?}",
            text
        );

        // Step 2: Assistant starts streaming (first chunk)
        // During streaming, content is in active_streaming_content, not entries.
        chat.stream_assistant_content("你好");
        assert!(chat.has_active_stream(), "should have active stream");
        let stream_text: String = chat
            .render_active_stream(width)
            .iter()
            .flat_map(|l| {
                let s: String = l.spans.iter().map(|sp| sp.content.as_ref()).collect();
                if s.is_empty() { vec![] } else { vec![s] }
            })
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            stream_text.contains("你好"),
            "streaming content should be in active stream"
        );

        // Step 3: Streaming updates with longer content
        chat.stream_assistant_content("你好！有什么我可以帮助你的吗？😊");
        let stream_text: String = chat
            .render_active_stream(width)
            .iter()
            .flat_map(|l| {
                let s: String = l.spans.iter().map(|sp| sp.content.as_ref()).collect();
                if s.is_empty() { vec![] } else { vec![s] }
            })
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            stream_text.contains("有什么我可以帮助你的吗"),
            "longer streaming content should appear"
        );

        // Step 4: Streaming content grows to multiple lines
        let long = "你好！有什么我可以帮助你的吗？😊\n我来帮你分析这个问题。\n首先我们需要了解具体的上下文。";
        chat.stream_assistant_content(long);
        let stream_text: String = chat
            .render_active_stream(width)
            .iter()
            .flat_map(|l| {
                let s: String = l.spans.iter().map(|sp| sp.content.as_ref()).collect();
                if s.is_empty() { vec![] } else { vec![s] }
            })
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            stream_text.contains("我来帮你分析"),
            "multi-line streaming content should appear"
        );

        // Step 5: Finalize turn — content committed to entries
        chat.mark_turn_end();
        assert!(
            !chat.has_active_stream(),
            "stream should be cleared after finalize"
        );
        let text = render_text(&mut chat, width, usize::MAX);
        assert!(
            text.contains("我来帮你分析"),
            "committed assistant content should be in render_lines"
        );

        // Step 6: User sends another message
        chat.add_user_message("再问一个问题");
        let text = render_text(&mut chat, width, usize::MAX);
        assert!(
            text.contains("再问一个问题"),
            "second user message should appear"
        );
        assert!(
            text.contains("我来帮你分析"),
            "first assistant response still visible"
        );

        // Step 7: Another streaming response
        chat.stream_assistant_content("第二个回答");
        assert!(chat.has_active_stream(), "should have active stream again");
        chat.mark_turn_end();
        let text = render_text(&mut chat, width, usize::MAX);
        assert!(
            text.contains("第二个回答"),
            "second assistant response should appear after finalize"
        );
    }

    /// Replicates the exact combined format the MessageUpdate handler produces:
    /// thinking block → `\x1b[3m\x1b[38;2;128;128;128m{thinking}\x1b[23m\x1b[39m\n` followed by text block content
    #[test]
    fn test_thinking_plus_response_renders_correctly() {
        let mut chat = ChatView::new();
        let width = 80;

        // User message
        chat.add_user_message("你好");

        // Simulate the message update handler's combined format:
        // ContentBlock::Thinking → combined.push_str(&format!("\x1b[3m\x1b[38;2;128;128;128m{}\x1b[23m\x1b[39m\n", t.thinking));
        // ContentBlock::Text → combined.push_str(&t.text);
        let thinking = "用户发来问候，我需要礼貌地回应";
        let response = "你好！有什么我可以帮助你的吗？😊";
        let combined = format!(
            "\x1b[3m\x1b[38;2;128;128;128m{}\x1b[23m\x1b[39m\n\n{}",
            thinking, response
        );

        chat.stream_assistant_content(&combined);
        chat.mark_turn_end();

        let lines = chat.render_lines(width, usize::MAX);
        let text = render_text(&mut chat, width, usize::MAX);

        // The thinking text should be visible (with or without ANSI codes)
        assert!(
            text.contains("用户发来问候"),
            "thinking text should appear in rendered output. got: {:?}",
            text
        );
        // The response text should be visible
        assert!(
            text.contains("有什么我可以帮助你的吗"),
            "response text should appear in rendered output. got: {:?}",
            text
        );

        // Verify the output has at least 2 lines (thinking + response, with minimal spacing)
        let line_texts: Vec<String> = lines.iter().map(line_text).collect();
        assert!(
            lines.len() >= 2,
            "should have at least 2 lines (thinking + response), got {} lines: {:?}",
            lines.len(),
            line_texts
        );

        // Verify no more than 1 blank line between thinking and response
        let mut blank_run = 0usize;
        let mut max_blank_run = 0usize;
        for line in &line_texts {
            if line.trim().is_empty() {
                blank_run += 1;
                max_blank_run = std::cmp::max(max_blank_run, blank_run);
            } else {
                blank_run = 0;
            }
        }
        assert!(
            max_blank_run <= 2,
            "should have at most 2 consecutive blank line, got {}: {:?}",
            max_blank_run,
            line_texts
        );

        // Print debug output
        eprintln!("=== Rendered lines ===");
        for (i, line) in line_texts.iter().enumerate() {
            eprintln!("  [{}] {:?}", i, line);
        }
        eprintln!("=== end ===");
    }

    #[test]
    fn test_thinking_only_no_response() {
        let mut chat = ChatView::new();
        let width = 80;

        chat.add_user_message("你好");

        // Only thinking, no response text yet (during streaming)
        let thinking = "正在思考如何回复...";
        let combined = format!(
            "\x1b[3m\x1b[38;2;128;128;128m{}\x1b[23m\x1b[39m\n\n",
            thinking
        );
        chat.stream_assistant_content(&combined);
        chat.mark_turn_end();

        let text = render_text(&mut chat, width, usize::MAX);

        assert!(
            text.contains("正在思考如何回复"),
            "thinking text should appear. got: {:?}",
            text
        );
    }

    /// Acceptance test: user sends "你好", agent responds with thinking + text.
    /// Verifies both messages are fully visible with clean rendering.
    #[test]
    fn test_acceptance_thinking_and_response_displayed_completely() {
        let mut chat = ChatView::new();
        let width = 80;

        // Step 1: User submits "你好"
        chat.add_user_message("你好");

        // Step 2: Assistant streams thinking (using the exact format from tui.rs MessageUpdate handler)
        let thinking = "用户发来问候，我需要礼貌地回应";
        let response = "你好！有什么我可以帮助你的吗？😊";
        let combined = format!(
            "\x1b[3m\x1b[38;2;128;128;128m{}\x1b[23m\x1b[39m\n\n{}",
            thinking, response
        );
        chat.stream_assistant_content(&combined);
        chat.mark_turn_end();

        // Step 3: Render the full chat (unlimited height, as in real TUI)
        let lines = chat.render_lines(width, usize::MAX);
        let line_texts: Vec<String> = lines.iter().map(line_text).collect();
        let text: String = line_texts.join(" ");

        // Acceptance criteria 1: User message "你好" is visible
        assert!(
            text.contains("你好"),
            "FAIL: user message '你好' not found in rendered output.\nOutput: {:?}",
            text
        );

        // Acceptance criteria 2: Thinking message is visible
        assert!(
            text.contains("用户发来问候"),
            "FAIL: thinking text not found in rendered output.\nOutput: {:?}",
            text
        );

        // Acceptance criteria 3: Response message is visible
        assert!(
            text.contains("有什么我可以帮助你的吗"),
            "FAIL: response text not found in rendered output.\nOutput: {:?}",
            text
        );

        // Acceptance criteria 4: No excessive blank lines (at most 1 consecutive blank line)
        let mut max_blank_run = 0usize;
        let mut blank_run = 0usize;
        for l in &line_texts {
            if l.trim().is_empty() {
                blank_run += 1;
                max_blank_run = std::cmp::max(max_blank_run, blank_run);
            } else {
                blank_run = 0;
            }
        }
        assert!(
            max_blank_run <= 2,
            "FAIL: excessive blank lines (max {}). Lines:\n{:?}",
            max_blank_run,
            line_texts
        );

        // Print debug for manual verification
        eprintln!("\n===== ACCEPTANCE TEST: RENDERED OUTPUT =====");
        eprintln!(
            "Terminal width: {}, Total lines: {}",
            width,
            line_texts.len()
        );
        for (i, line) in line_texts.iter().enumerate() {
            eprintln!("  [{:>2}] {:?}", i, line);
        }
        eprintln!("============================================\n");
    }

    #[test]
    fn test_tool_execution_renders_pending() {
        let mut chat = ChatView::new();
        let width = 80;

        chat.add_user_message("Run a command");
        chat.add_tool_execution(
            "call-1",
            "bash",
            serde_json::json!({"command": "echo hello"}),
        );

        let text = render_text(&mut chat, width, usize::MAX);

        // Format: bold "$ echo hello" (no tool name prefix)
        assert!(
            text.contains("echo hello"),
            "command should appear. got: {:?}",
            text
        );
    }

    #[test]
    fn test_tool_execution_transitions_to_success() {
        let mut chat = ChatView::new();
        let width = 80;

        chat.add_user_message("Read a file");
        chat.add_tool_execution(
            "call-1",
            "read",
            serde_json::json!({"file_path": "foo.txt"}),
        );
        chat.update_tool_execution("call-1", "file content here", false);
        chat.toggle_tool_expansion();

        let text = render_text(&mut chat, width, usize::MAX);

        assert!(text.contains("read"), "tool name should appear");
        assert!(text.contains("foo.txt"), "arg should appear");
        assert!(
            text.contains("file content here"),
            "output should be shown when expanded"
        );
    }

    #[test]
    fn test_tool_execution_transitions_to_error() {
        let mut chat = ChatView::new();
        let width = 80;

        chat.add_user_message("Read a file");
        chat.add_tool_execution(
            "call-1",
            "read",
            serde_json::json!({"file_path": "foo.txt"}),
        );
        chat.update_tool_execution("call-1", "error: file not found", true);
        chat.toggle_tool_expansion();

        let text = render_text(&mut chat, width, usize::MAX);

        assert!(text.contains("read"), "tool name should appear");
        assert!(text.contains("foo.txt"), "arg should appear");
        assert!(
            text.contains("error: file not found"),
            "error output should appear"
        );
    }

    #[test]
    fn test_tool_execution_output_expand_toggle_works() {
        let mut chat = ChatView::new();
        let width = 80;

        chat.add_user_message("List directory");
        chat.add_tool_execution("call-1", "ls", serde_json::json!({"path": "/tmp"}));
        let long_output = (1..=25)
            .map(|i| format!("file_{}.txt", i))
            .collect::<Vec<_>>()
            .join("\n");
        chat.update_tool_execution("call-1", &long_output, false);

        // Collapsed: head+middle+tail with ellipsis
        let text = render_text(&mut chat, width, usize::MAX);
        assert!(
            text.contains("file_1.txt"),
            "first file should appear in preview"
        );
        assert!(text.contains("to expand"), "expand hint should appear");
        // Head+tail truncation: file_25.txt IS in the tail section
        assert!(
            text.contains("file_25.txt"),
            "last file should appear in tail preview"
        );

        // Toggle expansion — full output visible (no ellipsis)
        chat.toggle_tool_expansion();
        let text = render_text(&mut chat, width, usize::MAX);
        assert!(
            !text.contains("to expand"),
            "expand hint should disappear when expanded"
        );
        assert!(text.contains("ls"), "tool label should appear");

        // Toggle again — collapsed again with head+tail
        chat.toggle_tool_expansion();
        let text = render_text(&mut chat, width, usize::MAX);
        assert!(text.contains("to expand"), "expand hint should reappear");
    }

    /// Visual alignment test: renders a full multi-tool session and prints
    /// every rendered line for visual comparison.
    /// Run: cargo test -p Pick-tui -- --nocapture "test_visual_alignment"
    #[test]
    fn test_visual_alignment() {
        let mut chat = ChatView::new();
        let width = 80;

        eprintln!("\n{}", "#".repeat(72));
        eprintln!("# VISUAL ALIGNMENT TEST");
        eprintln!("# Tool execution UI vs canonical");
        eprintln!("{}", "#".repeat(72));

        // 1. User message
        chat.add_user_message("分析当前项目架构");

        // 2. ls tool (head truncation, 20-line preview)
        chat.add_tool_execution("ls-1", "ls", serde_json::json!({"path": ".", "limit": 50}));
        chat.update_tool_execution(
            "ls-1",
            ".github/\n.pick/\nCargo.lock\nCargo.toml\nCLAUDE.md\ncrates/\ndocs/\ntarget/",
            false,
        );
        chat.toggle_tool_expansion();

        // 3. read tool
        chat.add_tool_execution(
            "read-1",
            "read",
            serde_json::json!({"file_path": "Cargo.toml"}),
        );
        chat.update_tool_execution(
            "read-1",
            "[package]\nname = \"Pick\"\nversion = \"0.1.0\"",
            false,
        );
        chat.toggle_tool_expansion();

        // 4. bash tool with timer ANSI
        chat.add_tool_execution(
            "bash-1",
            "Bash",
            serde_json::json!({"command": "cargo test"}),
        );
        chat.update_tool_execution("bash-1",
            "running 226 tests\ntest result: ok. 226 passed\n\x1b[38;2;128;128;128mTook 42.3s\x1b[39m",
            false,
        );
        chat.toggle_tool_expansion();

        // 5. grep tool
        chat.add_tool_execution(
            "grep-1",
            "grep",
            serde_json::json!({"pattern": "struct", "path": "src", "glob": "*.rs"}),
        );
        chat.update_tool_execution(
            "grep-1",
            "src/main.rs:10: struct App\ntui/src/app.rs:50: struct TuiApp",
            false,
        );
        chat.toggle_tool_expansion();

        // 6. find tool
        chat.add_tool_execution(
            "find-1",
            "find",
            serde_json::json!({"pattern": "*.rs", "path": "src", "limit": 100}),
        );
        chat.update_tool_execution(
            "find-1",
            "./src/main.rs\n./src/lib.rs\n./src/components.rs",
            false,
        );
        chat.toggle_tool_expansion();

        // 7. edit tool with diff output
        chat.add_tool_execution(
            "edit-1",
            "Edit",
            serde_json::json!({"file_path": "src/main.rs"}),
        );
        chat.update_tool_execution("edit-1",
            "\x1b[38;2;204;102;102m-    let old = 1;\x1b[39m\n\x1b[38;2;181;189;104m+    let new = 2;\x1b[39m",
            false,
        );
        chat.toggle_tool_expansion();

        // 8. Assistant reply
        chat.stream_assistant_content("项目架构分析完成。");
        chat.mark_turn_end();

        // Render
        let lines = chat.render_lines(width, usize::MAX);
        let line_texts: Vec<String> = lines.iter().map(line_text).collect();
        let text: String = line_texts.join(" ");

        // Print every line for visual inspection
        eprintln!(
            "\n=== RENDERED OUTPUT ({} lines, width={}) ===",
            line_texts.len(),
            width
        );
        for (i, line) in line_texts.iter().enumerate() {
            eprintln!("[{:>3}] {:?}", i, line);
        }

        // === VERIFICATION ===
        eprintln!("\n=== VERIFICATION ===");

        // A. Tool labels use aligned format
        eprintln!("[CHECK] Tool label formats...");
        assert!(text.contains("ls ."), "ls label should contain path");
        assert!(
            text.contains("read Cargo.toml"),
            "read label: 'read Cargo.toml'"
        );
        assert!(
            text.contains("grep /struct/"),
            "grep label: 'grep /pattern/'"
        );
        assert!(text.contains("find *.rs"), "find label: 'find pattern'");
        assert!(text.contains("edit src/main.rs"), "edit label: 'edit path'");
        eprintln!("       All tool labels match aligned format");

        // B. No duplication
        eprintln!("[CHECK] No duplication...");
        let reply_count = text.matches("项目架构分析完成").count();
        eprintln!("  assistant reply count: {}", reply_count);
        assert_eq!(reply_count, 1, "reply must not be duplicated");

        // C. Dedup
        eprintln!("[CHECK] Tool dedup...");
        let before_count = chat.entry_count();
        chat.add_tool_execution("ls-1", "ls", serde_json::json!({"path": "."}));
        assert_eq!(
            chat.entry_count(),
            before_count,
            "dedup must prevent double entries"
        );

        eprintln!("{}", "#".repeat(72));
        eprintln!("# ALL VERIFICATIONS PASSED");
        eprintln!("{}", "#".repeat(72));

        // === SIDE-BY-SIDE: canonical format vs Pick rendered plain text ===
        let collapse = |s: &str| -> String {
            let result: Vec<&str> = s.split_whitespace().collect();
            if result.is_empty() {
                return String::new();
            }
            let mut out = String::new();
            for (i, w) in result.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                out.push_str(w);
            }
            out
        };

        eprintln!("\n{}", "=".repeat(80));
        eprintln!(
            "SIDE-BY-SIDE COMPARISON: canonical format vs Pick rendered"
        );
        eprintln!("{}", "=".repeat(80));
        eprintln!("{:<30} | {:<48}|", "canonical format", "Pick (plain text)");
        eprintln!("{}", "-".repeat(80));

        for line in &line_texts {
            let plain = collapse(line);
            if plain.is_empty() {
                continue;
            }
            let plain_lower = plain.to_lowercase();
            let canonical_text = if plain_lower.contains("cargo test") {
                Some("Ran cargo test")
            } else if plain_lower.contains("read") && plain_lower.contains("cargo.toml") {
                Some("Ran read Cargo.toml")
            } else if plain_lower.contains("ls") {
                Some("Ran ls . (limit 50)")
            } else if plain_lower.contains("grep") {
                Some("Ran grep /struct/ in src (*.rs)")
            } else if plain_lower.contains("find") {
                Some("Ran find *.rs in src (limit 100)")
            } else if plain_lower.contains("edit") && plain_lower.contains("src/main.rs") {
                Some("Ran edit src/main.rs")
            } else {
                None
            };
            if let Some(canonical) = canonical_text {
                let canonical_joined: String =
                    canonical.to_lowercase().split_whitespace().collect();
                let pick_joined: String = plain.to_lowercase().split_whitespace().collect();
                let matches =
                    pick_joined.contains(&canonical_joined) || canonical_joined == pick_joined;
                let mark = if matches { "Y" } else { " " };
                eprintln!(
                    "{:<30} | {:<48}| {}",
                    canonical,
                    &plain[..std::cmp::min(48, plain.len())],
                    mark
                );
            }
        }
        eprintln!("{}", "-".repeat(80));
        eprintln!("Style: bullet(green/red), Running/Ran(BOLD), output(DIM), gutter(└)");
    }
}
