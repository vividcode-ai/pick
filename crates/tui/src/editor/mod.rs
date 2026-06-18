//! Multi-line text editor with Emacs-style keybindings

use unicode_width::UnicodeWidthStr;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::autocomplete::{AutocompleteProvider, AutocompleteSuggestions};
use crate::kill_ring::KillRing;
use crate::undo_stack::UndoStack;

mod types;
pub use types::*;

/// Multi-line text editor state
pub struct Editor {
    /// Full text buffer
    pub buffer: String,
    /// Cursor byte offset into buffer
    pub cursor: usize,
    /// Selection start offset (None if no selection)
    pub mark: Option<usize>,
    /// Scroll offset in lines from top
    pub scroll_offset: usize,
    /// Kill ring for yank operations
    pub kill_ring: KillRing,
    /// Undo stack
    pub undo_stack: UndoStack<String>,
    /// Whether the editor is in the middle of a kill operation (for accumulation)
    kill_accumulating: bool,
    /// Whether the last kill was backward (for prepend)
    kill_prepend: bool,
    /// Prompt string displayed before the text
    pub prompt: String,
    /// Whether to show a prompt prefix
    pub show_prompt: bool,
    /// Placeholder text when buffer is empty
    pub placeholder: String,
    /// Size of last yank, for yank-pop cycling
    last_yank_size: Option<usize>,
    /// Redo stack (undo pushes here before restoring)
    redo_stack: UndoStack<String>,
    /// Large paste placeholders (placeholder_text ↔ actual_content mappings)
    pub pending_pastes: Vec<PendingPaste>,

    // --- Autocomplete ---
    /// Optional autocomplete provider for slash commands and file paths
    autocomplete_provider: Option<Box<dyn AutocompleteProvider>>,
    /// Current autocomplete suggestions (None when not active)
    autocomplete_suggestions: Option<AutocompleteSuggestions>,
    /// Currently selected suggestion index
    autocomplete_selection: usize,
    /// Whether to show the autocomplete popup
    autocomplete_visible: bool,

    // --- Input history ---
    /// Previously submitted input texts (most recent last)
    pub input_history: Vec<String>,
    /// Current position when browsing history (None = not browsing)
    pub history_index: Option<usize>,
    /// Saved current input buffer when entering history browsing
    staging_buffer: String,

    // --- Pending messages browsing ---
    /// Index into pending_user_messages when browsing (None = not browsing)
    pub pending_index: Option<usize>,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            mark: None,
            scroll_offset: 0,
            kill_ring: KillRing::new(),
            undo_stack: UndoStack::new(),
            kill_accumulating: false,
            kill_prepend: false,
            prompt: String::new(),
            show_prompt: false,
            placeholder: String::new(),
            last_yank_size: None,
            redo_stack: UndoStack::new(),
            pending_pastes: Vec::new(),
            autocomplete_provider: None,
            autocomplete_suggestions: None,
            autocomplete_selection: 0,
            autocomplete_visible: false,
            input_history: Vec::new(),
            history_index: None,
            staging_buffer: String::new(),
            pending_index: None,
        }
    }

    /// Push current state onto undo stack
    fn push_undo(&mut self) {
        self.undo_stack.push(&self.buffer.clone());
    }

    // --- Insertion ---

    /// Insert a character at cursor position
    pub fn insert_char(&mut self, c: char) {
        self.push_undo();
        self.buffer.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.mark = None;
        self.kill_accumulating = false;
        // Auto-trigger autocomplete when buffer starts with "/" (no space yet)
        if self.autocomplete_provider.is_some() {
            let trimmed = self.buffer.trim_start();
            if trimmed.starts_with('/') && !trimmed[1..].contains(' ') {
                self.trigger_autocomplete();
            } else {
                self.cancel_autocomplete();
            }
        }
    }

    /// Insert a string at cursor position
    pub fn insert_str(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.push_undo();
        self.buffer.insert_str(self.cursor, s);
        self.cursor += s.len();
        self.mark = None;
        self.kill_accumulating = false;
    }

    /// Insert a paste placeholder into the buffer for a large paste.
    /// Uses `[Pasted Content N chars]` format, with dedup suffix (#2, #3…) for multiple pastes.
    pub fn add_paste_placeholder(&mut self, text_len: usize, actual: &str) {
        let base = format!("[Pasted Content {} chars]", text_len);
        let placeholder = self.unique_placeholder(&base);
        self.insert_str(&placeholder);
        self.pending_pastes.push(PendingPaste {
            placeholder,
            actual: actual.to_string(),
        });
    }

    /// Generate a unique placeholder string, adding `#2`, `#3` … if duplicates exist.
    fn unique_placeholder(&self, base: &str) -> String {
        let mut max_suffix = 0usize;
        for pp in &self.pending_pastes {
            if pp.placeholder == base {
                max_suffix = max_suffix.max(1);
            }
            if let Some(suffix) = pp.placeholder.strip_prefix(&format!("{} #", base)) {
                if let Ok(v) = suffix.parse::<usize>() {
                    max_suffix = max_suffix.max(v);
                }
            }
        }
        if max_suffix == 0 {
            base.to_string()
        } else {
            format!("{} #{}", base, max_suffix + 1)
        }
    }

    /// Expand all paste placeholders in `text` to their actual content.
    pub fn expand_pending_pastes(&self, text: &str) -> String {
        let mut result = text.to_string();
        for pp in &self.pending_pastes {
            result = result.replace(&pp.placeholder, &pp.actual);
        }
        result
    }

    /// Insert a newline at cursor position
    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    /// Insert auto-indent (newline + matching indent)
    pub fn insert_newline_auto_indent(&mut self) {
        self.push_undo();
        // Get current line's leading whitespace
        let line_start = self.buffer[..self.cursor]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let line_prefix = &self.buffer[line_start..self.cursor];
        let indent: String = line_prefix
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();

        self.buffer.insert(self.cursor, '\n');
        self.cursor += 1;
        self.buffer.insert_str(self.cursor, &indent);
        self.cursor += indent.len();
        self.mark = None;
        self.kill_accumulating = false;
    }

    // --- Cursor movement ---

    /// Move cursor up one line (visual)
    pub fn cursor_up(&mut self) {
        let (row, col) = self.cursor_row_col();
        if row == 0 {
            self.cursor = 0;
            return;
        }
        let target_col = self.visual_col_at_line(row, col);
        let prev_line_start = self.buffer[..self.cursor]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        if prev_line_start == 0 {
            self.cursor = 0;
            return;
        }
        let prev_prev_line_start = self.buffer[..prev_line_start.saturating_sub(1)]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        self.cursor = self.offset_from_visual_col(
            prev_prev_line_start,
            prev_line_start.saturating_sub(1),
            target_col,
        );
        self.mark = None;
        self.kill_accumulating = false;
    }

    /// Move cursor down one line (visual)
    pub fn cursor_down(&mut self) {
        let (row, col) = self.cursor_row_col();
        let target_col = self.visual_col_at_line(row, col);
        let next_line_start = self.cursor
            + self.buffer[self.cursor..]
                .find('\n')
                .map(|i| i + 1)
                .unwrap_or(self.buffer.len() - self.cursor);
        if next_line_start >= self.buffer.len() {
            self.cursor = self.buffer.len();
            return;
        }
        let next_next_line_start = next_line_start
            + self.buffer[next_line_start..]
                .find('\n')
                .map(|i| i + 1)
                .unwrap_or(self.buffer.len() - next_line_start);
        self.cursor = self.offset_from_visual_col(
            next_line_start,
            next_next_line_start.saturating_sub(1),
            target_col,
        );
        self.mark = None;
        self.kill_accumulating = false;
    }

    /// Move cursor left by one character
    pub fn cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.cursor.saturating_sub(1);
            while self.buffer.is_char_boundary(self.cursor) == false {
                self.cursor = self.cursor.saturating_sub(1);
            }
        }
        self.mark = None;
        self.kill_accumulating = false;
    }

    /// Move cursor right by one character
    pub fn cursor_right(&mut self) {
        if self.cursor < self.buffer.len() {
            self.cursor += 1;
            while self.cursor < self.buffer.len()
                && self.buffer.is_char_boundary(self.cursor) == false
            {
                self.cursor += 1;
            }
        }
        self.mark = None;
        self.kill_accumulating = false;
    }

    /// Move cursor left by one word
    pub fn cursor_word_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        // Skip non-alphanumeric before cursor
        let mut pos = self.cursor;
        while pos > 0 {
            let prev = self.prev_char_boundary(pos);
            if self.buffer[prev..pos]
                .chars()
                .next()
                .map_or(false, |c| c.is_alphanumeric() || c == '_')
            {
                break;
            }
            pos = prev;
        }
        // Skip to start of word
        while pos > 0 {
            let prev = self.prev_char_boundary(pos);
            if !self.buffer[prev..pos]
                .chars()
                .next()
                .map_or(false, |c| c.is_alphanumeric() || c == '_')
            {
                break;
            }
            pos = prev;
        }
        self.cursor = pos;
        self.mark = None;
        self.kill_accumulating = false;
    }

    /// Move cursor right by one word
    pub fn cursor_word_right(&mut self) {
        let mut pos = self.cursor;
        let len = self.buffer.len();
        // Skip alphanumeric
        while pos < len {
            let c = self.buffer[pos..].chars().next().unwrap();
            if !c.is_alphanumeric() && c != '_' {
                break;
            }
            pos += c.len_utf8();
        }
        // Skip non-alphanumeric to next word start
        while pos < len {
            let c = self.buffer[pos..].chars().next().unwrap();
            if c.is_alphanumeric() || c == '_' {
                break;
            }
            pos += c.len_utf8();
            if c == '\n' {
                break;
            }
        }
        self.cursor = pos;
        self.mark = None;
        self.kill_accumulating = false;
    }

    /// Move cursor to start of line
    pub fn cursor_line_start(&mut self) {
        let line_start = self.buffer[..self.cursor]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        self.cursor = line_start;
        self.mark = None;
        self.kill_accumulating = false;
    }

    /// Move cursor to end of line
    pub fn cursor_line_end(&mut self) {
        let line_end = self.buffer[self.cursor..]
            .find('\n')
            .map(|i| self.cursor + i)
            .unwrap_or(self.buffer.len());
        self.cursor = line_end;
        self.mark = None;
        self.kill_accumulating = false;
    }

    /// Move cursor to start of buffer
    pub fn cursor_buffer_start(&mut self) {
        self.cursor = 0;
        self.mark = None;
        self.kill_accumulating = false;
    }

    /// Move cursor to end of buffer
    pub fn cursor_buffer_end(&mut self) {
        self.cursor = self.buffer.len();
        self.mark = None;
        self.kill_accumulating = false;
    }

    /// Move cursor forward by paragraph (empty-line delimited)
    pub fn jump_forward(&mut self) {
        let mut pos = self.cursor;
        let len = self.buffer.len();
        // Skip current line
        if let Some(nl) = self.buffer[pos..].find('\n') {
            pos += nl + 1;
        } else {
            self.cursor = len;
            return;
        }
        // Skip empty lines
        while pos < len {
            let rest = &self.buffer[pos..];
            if rest.starts_with('\n') {
                pos += 1;
            } else if rest.starts_with("\r\n") {
                pos += 2;
            } else {
                break;
            }
        }
        // Skip to next empty line
        while pos < len {
            let rest = &self.buffer[pos..];
            if rest.starts_with('\n') || rest.starts_with("\r\n") {
                break;
            }
            pos += 1;
        }
        self.cursor = std::cmp::min(pos, len);
        self.mark = None;
        self.kill_accumulating = false;
    }

    /// Move cursor backward by paragraph (empty-line delimited)
    pub fn jump_backward(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut pos = self.cursor;
        // Skip current line backwards
        let line_start = self.buffer[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
        if line_start > 0 {
            pos = line_start - 1; // before the \n
        } else {
            pos = 0;
        }
        // Skip empty lines backwards
        while pos > 0 {
            let rest = &self.buffer[..pos];
            if rest.ends_with('\n') {
                pos -= 1;
            } else if rest.ends_with("\r\n") {
                pos = pos.saturating_sub(2);
            } else {
                break;
            }
        }
        // Skip to previous empty line
        while pos > 0 {
            let rest = &self.buffer[..pos];
            if rest.ends_with('\n') {
                break;
            }
            pos -= 1;
        }
        self.cursor = pos;
        self.mark = None;
        self.kill_accumulating = false;
    }

    /// Move cursor up by one page
    pub fn page_up(&mut self) {
        for _ in 0..10 {
            let prev = self.cursor;
            self.cursor_up();
            if self.cursor == prev {
                break;
            }
        }
        self.mark = None;
        self.kill_accumulating = false;
    }

    /// Move cursor down by one page
    pub fn page_down(&mut self) {
        for _ in 0..10 {
            let prev = self.cursor;
            self.cursor_down();
            if self.cursor == prev {
                break;
            }
        }
        self.mark = None;
        self.kill_accumulating = false;
    }

    // --- Deletion ---

    /// Delete character before cursor (backspace)
    pub fn delete_before(&mut self) {
        if self.cursor == 0 {
            return;
        }
        // If there's a selection, delete it
        if self.mark.is_some() {
            self.delete_selection();
            return;
        }
        self.push_undo();
        let prev = self.prev_char_boundary(self.cursor);
        let deleted = self.buffer[prev..self.cursor].to_string();
        self.buffer.drain(prev..self.cursor);
        self.cursor = prev;
        self.kill_accumulating = false;
        // Kill non-whitespace for accumulation
        if deleted.chars().any(|c| !c.is_whitespace()) {
            self.kill_ring.push(&deleted, true, self.kill_accumulating);
            self.kill_accumulating = true;
            self.kill_prepend = true;
        } else {
            self.kill_accumulating = false;
        }
    }

    /// Delete N characters before cursor (for retroactive paste capture).
    /// Does NOT push undo — these chars are about to be re-inserted as a paste.
    pub fn delete_last_chars(&mut self, n: usize) {
        for _ in 0..n {
            if self.cursor == 0 {
                break;
            }
            let prev = self.prev_char_boundary(self.cursor);
            self.buffer.drain(prev..self.cursor);
            self.cursor = prev;
        }
        self.mark = None;
        self.kill_accumulating = false;
    }

    /// Delete character after cursor (delete)
    pub fn delete_after(&mut self) {
        if self.cursor >= self.buffer.len() {
            return;
        }
        if self.mark.is_some() {
            self.delete_selection();
            return;
        }
        self.push_undo();
        let next = self.next_char_boundary(self.cursor);
        let deleted = self.buffer[self.cursor..next].to_string();
        self.buffer.drain(self.cursor..next);
        self.kill_ring.push(&deleted, false, self.kill_accumulating);
        self.kill_accumulating = true;
        self.kill_prepend = false;
    }

    /// Delete word before cursor
    pub fn delete_word_before(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.push_undo();
        let word_start = self.find_word_start_before(self.cursor);
        let deleted = self.buffer[word_start..self.cursor].to_string();
        self.buffer.drain(word_start..self.cursor);
        self.cursor = word_start;
        self.kill_ring.push(&deleted, true, true);
        self.kill_accumulating = true;
        self.kill_prepend = true;
    }

    /// Delete word after cursor
    pub fn delete_word_after(&mut self) {
        if self.cursor >= self.buffer.len() {
            return;
        }
        self.push_undo();
        let word_end = self.find_word_end_after(self.cursor);
        let deleted = self.buffer[self.cursor..word_end].to_string();
        self.buffer.drain(self.cursor..word_end);
        self.kill_ring.push(&deleted, false, true);
        self.kill_accumulating = true;
        self.kill_prepend = false;
    }

    /// Delete from cursor to start of line
    pub fn delete_to_line_start(&mut self) {
        let line_start = self.buffer[..self.cursor]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        if line_start == self.cursor {
            return;
        }
        self.push_undo();
        let deleted = self.buffer[line_start..self.cursor].to_string();
        self.buffer.drain(line_start..self.cursor);
        self.cursor = line_start;
        self.kill_ring.push(&deleted, true, true);
        self.kill_accumulating = true;
        self.kill_prepend = true;
    }

    /// Delete from cursor to end of line
    pub fn delete_to_line_end(&mut self) {
        let line_end = self.buffer[self.cursor..]
            .find('\n')
            .map(|i| self.cursor + i)
            .unwrap_or(self.buffer.len());
        if line_end == self.cursor {
            return;
        }
        self.push_undo();
        let deleted = self.buffer[self.cursor..line_end].to_string();
        self.buffer.drain(self.cursor..line_end);
        self.kill_ring.push(&deleted, false, true);
        self.kill_accumulating = true;
        self.kill_prepend = false;
    }

    /// Delete current line and add to kill ring
    pub fn delete_line(&mut self) {
        let line_start = self.buffer[..self.cursor]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let line_end = self.buffer[self.cursor..]
            .find('\n')
            .map(|i| self.cursor + i + 1)
            .unwrap_or(self.buffer.len());
        if line_start == line_end {
            return;
        }
        self.push_undo();
        let deleted = self.buffer[line_start..line_end].to_string();
        self.buffer.drain(line_start..line_end);
        self.cursor = std::cmp::min(line_start, self.buffer.len());
        self.kill_ring.push(&deleted, false, false);
        self.kill_accumulating = false;
    }

    // --- Selection ---

    /// Set selection mark at current cursor position
    pub fn set_mark(&mut self) {
        self.mark = Some(self.cursor);
    }

    /// Get selected text, if any
    pub fn selected_text(&self) -> Option<&str> {
        let mark = self.mark?;
        if mark == self.cursor {
            return None;
        }
        let start = std::cmp::min(mark, self.cursor);
        let end = std::cmp::max(mark, self.cursor);
        Some(&self.buffer[start..end])
    }

    /// Delete the current selection
    fn delete_selection(&mut self) {
        let mark = match self.mark {
            Some(m) => m,
            None => return,
        };
        if mark == self.cursor {
            self.mark = None;
            return;
        }
        self.push_undo();
        let start = std::cmp::min(mark, self.cursor);
        let end = std::cmp::max(mark, self.cursor);
        self.buffer.drain(start..end);
        self.cursor = start;
        self.mark = None;
        self.kill_accumulating = false;
    }

    /// Copy selection to clipboard (kill ring without deleting)
    pub fn copy_selection(&mut self) {
        let text = self.selected_text().map(|s| s.to_string());
        if let Some(t) = text {
            self.kill_ring.push(&t, false, false);
            self.kill_accumulating = false;
        }
    }

    /// Cut selection (kill ring + delete)
    pub fn cut_selection(&mut self) {
        let text = self.selected_text().map(|s| s.to_string());
        if let Some(t) = text {
            self.push_undo();
            self.kill_ring.push(&t, false, false);
            self.delete_selection();
            self.kill_accumulating = false;
        }
    }

    // --- Yank ---

    /// Yank (paste) the most recent kill ring entry at cursor
    pub fn yank(&mut self) {
        let text = match self.kill_ring.peek() {
            Some(t) => t.to_string(),
            None => return,
        };
        self.push_undo();
        self.buffer.insert_str(self.cursor, &text);
        // Position cursor after the yanked text, set mark at start
        let mark_pos = self.cursor;
        self.cursor += text.len();
        self.mark = Some(mark_pos);
        self.kill_accumulating = false;
        self.last_yank_size = Some(text.len());
    }

    /// Yank-pop: cycle to the previous kill ring entry, replacing last yank
    pub fn yank_pop(&mut self) {
        let yank_size = match self.last_yank_size {
            Some(s) => s,
            None => return,
        };
        let mark = match self.mark {
            Some(m) => m,
            None => return,
        };
        // Remove the previous yank
        let start = std::cmp::min(mark, self.cursor);
        let end = std::cmp::max(mark, self.cursor);
        if end - start != yank_size {
            return;
        }
        self.buffer.drain(start..end);
        self.cursor = start;
        // Rotate kill ring and insert next entry
        self.kill_ring.rotate();
        match self.kill_ring.peek() {
            Some(t) => {
                self.buffer.insert_str(self.cursor, t);
                self.mark = Some(self.cursor);
                self.cursor += t.len();
                self.last_yank_size = Some(t.len());
            }
            None => {
                self.mark = None;
                self.last_yank_size = None;
            }
        }
    }

    // --- Undo ---

    /// Undo the last edit
    pub fn undo(&mut self) {
        if let Some(previous) = self.undo_stack.pop() {
            // Push current state to redo
            self.redo_stack.push(&self.buffer.clone());
            self.buffer = previous;
            self.cursor = std::cmp::min(self.cursor, self.buffer.len());
            self.mark = None;
            self.kill_accumulating = false;
        }
    }

    /// Redo the last undone edit
    pub fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(&self.buffer.clone());
            self.buffer = next;
            self.cursor = std::cmp::min(self.cursor, self.buffer.len());
            self.mark = None;
            self.kill_accumulating = false;
        }
    }

    // --- Tab / Indentation ---

    /// Insert tab (4 spaces)
    pub fn insert_tab(&mut self) {
        self.insert_str("    ");
    }

    // --- Autocomplete ---

    /// Set the autocomplete provider
    pub fn set_autocomplete_provider(&mut self, provider: Box<dyn AutocompleteProvider>) {
        self.autocomplete_provider = Some(provider);
    }

    /// Trigger autocomplete based on current text before cursor
    pub fn trigger_autocomplete(&mut self) {
        let provider = match &self.autocomplete_provider {
            Some(p) => p,
            None => return,
        };
        let text_before_cursor = &self.buffer[..self.cursor];
        let suggestions = provider.get_suggestions(text_before_cursor, false);
        match suggestions {
            Some(s) => {
                self.autocomplete_suggestions = Some(s);
                self.autocomplete_selection = 0;
                self.autocomplete_visible = true;
            }
            None => {
                self.cancel_autocomplete();
            }
        }
    }

    /// Cycle autocomplete selection to the next item
    pub fn autocomplete_next(&mut self) {
        if let Some(ref suggestions) = self.autocomplete_suggestions {
            if suggestions.items.len() > 1 {
                self.autocomplete_selection =
                    (self.autocomplete_selection + 1) % suggestions.items.len();
            }
        }
    }

    /// Cycle autocomplete selection to the previous item
    pub fn autocomplete_previous(&mut self) {
        if let Some(ref suggestions) = self.autocomplete_suggestions {
            if suggestions.items.len() > 1 {
                self.autocomplete_selection = if self.autocomplete_selection == 0 {
                    suggestions.items.len() - 1
                } else {
                    self.autocomplete_selection - 1
                };
            }
        }
    }

    /// Apply the currently selected autocomplete completion.
    /// Returns true if a completion was applied.
    pub fn autocomplete_apply_completion(&mut self) -> bool {
        let (suggestions, provider) =
            match (&self.autocomplete_suggestions, &self.autocomplete_provider) {
                (Some(s), Some(p)) => (s, p),
                _ => return false,
            };
        if suggestions.items.is_empty() || self.autocomplete_selection >= suggestions.items.len() {
            return false;
        }
        let item = &suggestions.items[self.autocomplete_selection];
        let text_before = &self.buffer[..self.cursor];
        let text_after = &self.buffer[self.cursor..];
        let (new_text, new_cursor) =
            provider.apply_completion(text_before, text_after, item, &suggestions.prefix);
        self.push_undo();
        self.buffer = new_text;
        self.cursor = new_cursor;
        self.mark = None;
        self.cancel_autocomplete();
        true
    }

    /// Cancel autocomplete and clear suggestions
    pub fn cancel_autocomplete(&mut self) {
        self.autocomplete_suggestions = None;
        self.autocomplete_selection = 0;
        self.autocomplete_visible = false;
    }

    /// Whether autocomplete is currently active
    pub fn is_autocomplete_active(&self) -> bool {
        self.autocomplete_visible && self.autocomplete_suggestions.is_some()
    }

    /// Number of extra lines needed to display autocomplete suggestions
    pub fn autocomplete_line_count(&self) -> usize {
        if !self.autocomplete_visible {
            return 0;
        }
        match &self.autocomplete_suggestions {
            Some(s) => std::cmp::min(s.items.len(), 8),
            None => 0,
        }
    }

    /// Dedent the current line (remove 4 spaces from start)
    pub fn dedent(&mut self) {
        let line_start = self.buffer[..self.cursor]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let line_text = &self.buffer[line_start..];
        let spaces = line_text.chars().take_while(|c| *c == ' ').count();
        let remove = std::cmp::min(spaces, 4);
        if remove > 0 {
            self.push_undo();
            self.buffer.drain(line_start..line_start + remove);
            if self.cursor > line_start {
                self.cursor = self.cursor.saturating_sub(remove);
            }
            self.kill_accumulating = false;
        }
    }

    // --- Buffer management ---

    /// Clear the entire editor buffer
    pub fn clear(&mut self) {
        self.push_undo();
        self.buffer.clear();
        self.cursor = 0;
        self.mark = None;
        self.scroll_offset = 0;
        self.kill_accumulating = false;
        self.cancel_autocomplete();
    }

    /// Set the buffer content and reset cursor
    pub fn set_text(&mut self, text: &str) {
        self.push_undo();
        self.buffer = text.to_string();
        self.cursor = self.buffer.len();
        self.mark = None;
        self.scroll_offset = 0;
        self.kill_accumulating = false;
    }

    /// Get the buffer content
    pub fn text(&self) -> &str {
        &self.buffer
    }

    // --- Input history ---

    /// Maximum number of history entries to keep
    const MAX_HISTORY: usize = 100;

    /// Push a submitted text into input history.
    /// Deduplicates against the most recent entry and limits to MAX_HISTORY.
    /// Resets browsing state back to current input.
    pub fn push_history(&mut self, text: String) {
        if text.is_empty() {
            return;
        }
        let trimmed = text.trim().to_string();
        if trimmed.is_empty() {
            return;
        }
        if self.input_history.last().map(|s| s.as_str()) == Some(trimmed.as_str()) {
            return;
        }
        self.input_history.push(trimmed);
        if self.input_history.len() > Self::MAX_HISTORY {
            self.input_history.remove(0);
        }
        self.history_index = None;
        self.staging_buffer.clear();
    }

    /// Navigate backward in input history.
    /// Saves current buffer into staging_buffer if entering history mode.
    pub fn history_previous(&mut self) {
        if self.input_history.is_empty() {
            return;
        }
        let text = match self.history_index {
            None => {
                self.staging_buffer = self.buffer.clone();
                let idx = self.input_history.len() - 1;
                self.history_index = Some(idx);
                self.input_history[idx].clone()
            }
            Some(i) if i > 0 => {
                let idx = i - 1;
                self.history_index = Some(idx);
                self.input_history[idx].clone()
            }
            _ => return,
        };
        self.set_text(&text);
    }

    /// Navigate forward in input history.
    /// Restores staging_buffer when past the newest entry.
    pub fn history_next(&mut self) {
        let idx = match self.history_index {
            None => return,
            Some(i) => i,
        };
        if idx + 1 >= self.input_history.len() {
            self.history_index = None;
            if self.staging_buffer.is_empty() {
                self.clear();
            } else {
                let text = self.staging_buffer.clone();
                self.staging_buffer.clear();
                self.set_text(&text);
            }
        } else {
            let next = idx + 1;
            self.history_index = Some(next);
            let text = self.input_history[next].clone();
            self.set_text(&text);
        }
    }

    // --- Pending message browsing ---

    /// Navigate backward (up) through pending user messages.
    pub fn pending_previous(&mut self, msgs: &[String]) {
        if msgs.is_empty() {
            return;
        }
        let idx = match self.pending_index {
            None => {
                // Save current buffer the first time
                self.staging_buffer = self.buffer.clone();
                msgs.len() - 1
            }
            Some(i) if i > 0 => i - 1,
            _ => return,
        };
        self.pending_index = Some(idx);
        self.set_text(&msgs[idx]);
    }

    /// Navigate forward (down) through pending user messages.
    pub fn pending_next(&mut self, msgs: &[String]) {
        let idx = match self.pending_index {
            None => return,
            Some(i) => i,
        };
        if idx + 1 >= msgs.len() {
            self.pending_index = None;
            if self.staging_buffer.is_empty() {
                self.clear();
            } else {
                let text = self.staging_buffer.clone();
                self.staging_buffer.clear();
                self.set_text(&text);
            }
        } else {
            let next = idx + 1;
            self.pending_index = Some(next);
            self.set_text(&msgs[next]);
        }
    }

    // --- Line / column computation ---

    /// Return (row, col) for current cursor, where row is 0-indexed line number
    /// and col is byte offset from start of that line
    pub fn cursor_row_col(&self) -> (usize, usize) {
        let line_start = self.buffer[..self.cursor]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let row = self.buffer[..line_start].matches('\n').count();
        let col = self.cursor - line_start;
        (row, col)
    }

    /// Returns visual column (accounting for tabs) at the given line
    /// using the buffer's column offset
    fn visual_col_at_line(&self, row: usize, _col: usize) -> usize {
        // Find the line start and compute visual column from there
        let mut line_starts = 0;
        let mut line_start_offset = 0;
        for (i, c) in self.buffer.char_indices() {
            if line_starts == row {
                line_start_offset = i;
                break;
            }
            if c == '\n' {
                line_starts += 1;
            }
        }
        let col_in_line = self.cursor - line_start_offset;
        // Count visual width (tabs = 4)
        let mut visual = 0;
        for c in self.buffer[line_start_offset..line_start_offset + col_in_line].chars() {
            if c == '\t' {
                visual = (visual + 4) & !3; // round up to next tab stop
            } else {
                visual += 1;
            }
        }
        visual
    }

    /// Compute byte offset from visual column position within a line
    fn offset_from_visual_col(
        &self,
        line_start: usize,
        line_end: usize,
        target_visual_col: usize,
    ) -> usize {
        let mut visual = 0;
        let mut offset = line_start;
        for (i, c) in self.buffer[line_start..line_end].char_indices() {
            let c_width = if c == '\t' { 4 - (visual % 4) } else { 1 };
            if visual + c_width > target_visual_col {
                break;
            }
            visual += c_width;
            offset = line_start + i + c.len_utf8();
        }
        offset
    }

    /// Number of lines in the buffer
    pub fn line_count(&self) -> usize {
        if self.buffer.is_empty() {
            return 1;
        }
        // Each \n starts a new line (editor convention, not Unix file convention)
        self.buffer.matches('\n').count() + 1
    }

    /// Number of visual lines when wrapped to `width`, accounting for
    /// display-width wrapping of long lines.
    pub fn visual_line_count(&self, width: usize) -> usize {
        if self.buffer.is_empty() {
            return 1;
        }
        let mut count = 0usize;
        let mut pos = 0;
        loop {
            if pos >= self.buffer.len() {
                break;
            }
            let nl = self.buffer[pos..]
                .find('\n')
                .map(|i| pos + i)
                .unwrap_or(self.buffer.len());
            let line = &self.buffer[pos..nl];
            if width > 0 {
                let wrapped = wrap_by_display_width(line, width);
                count += wrapped.len();
            } else {
                count += 1;
            }
            pos = nl;
            if nl < self.buffer.len() {
                pos += 1;
            }
        }
        count
    }

    /// Get a specific line (0-indexed)
    pub fn get_line(&self, index: usize) -> &str {
        let mut line_start = 0;
        let mut current = 0;
        for (i, c) in self.buffer.char_indices() {
            if current == index {
                line_start = i;
                break;
            }
            if c == '\n' {
                current += 1;
                if current == index {
                    line_start = i + 1;
                    break;
                }
            }
        }
        if current < index {
            return "";
        }
        let remaining = &self.buffer[line_start..];
        match remaining.find('\n') {
            Some(end) => &remaining[..end],
            None => remaining,
        }
    }

    // --- Word boundary helpers ---

    fn prev_char_boundary(&self, pos: usize) -> usize {
        let mut p = pos.saturating_sub(1);
        while p > 0 && !self.buffer.is_char_boundary(p) {
            p -= 1;
        }
        p
    }

    fn next_char_boundary(&self, pos: usize) -> usize {
        let mut p = pos + 1;
        while p < self.buffer.len() && !self.buffer.is_char_boundary(p) {
            p += 1;
        }
        p
    }

    fn find_word_start_before(&self, pos: usize) -> usize {
        let mut p = pos;
        // Skip non-alphanumeric before cursor
        while p > 0 {
            let prev = self.prev_char_boundary(p);
            let c = self.buffer[prev..p].chars().next().unwrap();
            if c.is_alphanumeric() || c == '_' {
                break;
            }
            p = prev;
        }
        // Find word start
        while p > 0 {
            let prev = self.prev_char_boundary(p);
            let c = self.buffer[prev..p].chars().next().unwrap();
            if !c.is_alphanumeric() && c != '_' {
                break;
            }
            p = prev;
        }
        p
    }

    fn find_word_end_after(&self, pos: usize) -> usize {
        let mut p = pos;
        let len = self.buffer.len();
        // Skip alphanumeric
        while p < len {
            let c = self.buffer[p..].chars().next().unwrap();
            if !c.is_alphanumeric() && c != '_' {
                break;
            }
            p += c.len_utf8();
        }
        // Skip non-alphanumeric (stop at word boundary or newline)
        while p < len {
            let c = self.buffer[p..].chars().next().unwrap();
            if c.is_alphanumeric() || c == '_' || c == '\n' {
                break;
            }
            p += c.len_utf8();
        }
        p
    }

    // --- Rendering ---

    /// Render the editor content into visual lines for a given width.
    /// Returns (visual_lines, cursor_row, cursor_col_in_last_row)
    /// where cursor position is within the rendered output.
    /// Produces ratatui Lines directly (no ANSI bridge needed).
    pub fn render(&self, width: usize, max_height: usize) -> (Vec<Line<'static>>, usize, usize) {
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut cursor_row = 0;
        let mut cursor_col = 0;

        // Compute the visible range based on scroll_offset
        let visible_top = self.scroll_offset;

        // Build visual lines from buffer
        let mut line_index = 0;
        let mut found_cursor = false;
        let cursor_line_start = self.buffer[..self.cursor]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let cursor_col_in_line = self.buffer[cursor_line_start..self.cursor].width();

        // Helper to add a plain text line
        let add_line = |lines: &mut Vec<Line<'static>>, text: &str| {
            lines.push(Line::from(Span::raw(text.to_string())));
        };

        // Iterate through buffer lines
        let mut buf_pos = 0;
        loop {
            if buf_pos >= self.buffer.len() {
                // Add trailing empty line only if one doesn't already exist
                if line_index >= visible_top && lines.len() < max_height {
                    let last_empty = lines.last().map_or(true, |l| {
                        l.spans.is_empty() || l.spans.iter().all(|s| s.content.as_ref().is_empty())
                    });
                    if !last_empty {
                        lines.push(Line::from(""));
                    }
                }
                // Check cursor on the trailing (possibly already added) line
                if !found_cursor {
                    cursor_row = lines.len().saturating_sub(1);
                    cursor_col = 0;
                    found_cursor = true;
                }
                break;
            }
            let remaining = &self.buffer[buf_pos..];
            let nl_pos = remaining.find('\n');
            let line_end = nl_pos.map(|i| buf_pos + i).unwrap_or(self.buffer.len());
            let line_text = &self.buffer[buf_pos..line_end];

            // Wrap long lines using display-width-aware splitting
            let wrapped: Vec<String> = if width > 0 && line_text.width() > width {
                wrap_by_display_width(line_text, width)
            } else {
                vec![line_text.to_string()]
            };

            for (wi, wline) in wrapped.iter().enumerate() {
                if line_index >= visible_top && lines.len() < max_height {
                    let is_cursor_line = buf_pos <= self.cursor && self.cursor <= line_end;
                    if is_cursor_line && wi == wrapped.len() - 1 {
                        cursor_row = lines.len();
                        cursor_col = std::cmp::min(cursor_col_in_line, wline.width());
                        found_cursor = true;
                    }
                    add_line(&mut lines, wline);
                }
                line_index += 1;
            }

            buf_pos = line_end;
            if nl_pos.is_some() {
                buf_pos += 1;
                if buf_pos >= self.buffer.len() {
                    if line_index >= visible_top && lines.len() < max_height {
                        let is_cursor_line = !found_cursor && self.cursor == buf_pos;
                        if is_cursor_line {
                            cursor_row = lines.len();
                            cursor_col = 0;
                            found_cursor = true;
                        }
                        lines.push(Line::from(""));
                    }
                    break;
                }
                line_index += 1;
            } else {
                break;
            }
        }

        if !found_cursor {
            if self.cursor <= cursor_line_start + cursor_col_in_line {
                cursor_row = 0;
                cursor_col = 0;
            }
        }

        // Trim trailing empty lines
        if !self.buffer.ends_with('\n') {
            while lines.len() > 1 {
                let is_empty = lines.last().map_or(true, |l| {
                    l.spans.is_empty() || l.spans.iter().all(|s| s.content.as_ref().is_empty())
                });
                if is_empty {
                    lines.pop();
                } else {
                    break;
                }
            }
        }

        // Prepend prompt prefix to the first line when show_prompt is true.
        // Also handles empty buffer: still show the prompt line so the user
        // sees "❯ " even before typing anything.
        if self.show_prompt {
            let prompt_width = self.prompt.width();
            if lines.is_empty() {
                // Empty buffer: show a line with just the prompt
                lines.push(Line::from(Span::styled(
                    self.prompt.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                )));
                if cursor_row == 0 && cursor_col == 0 {
                    cursor_col = prompt_width;
                }
            } else {
                if let Some(first) = lines.first_mut() {
                    let old_content: String =
                        first.spans.iter().map(|s| s.content.as_ref()).collect();
                    *first = Line::from(vec![
                        Span::styled(
                            self.prompt.clone(),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(old_content),
                    ]);
                }
                if cursor_row == 0 {
                    cursor_col += prompt_width;
                }
            }
        }

        // Apply dim + italic styling to paste placeholders
        if !self.pending_pastes.is_empty() {
            let placeholder_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM);
            for line in &mut lines {
                let mut new_spans = Vec::new();
                for span in std::mem::take(&mut line.spans) {
                    let content = span.content.as_ref().to_string();
                    // Check if this span's content contains any placeholder text
                    let mut remaining = content.as_str();
                    loop {
                        let mut earliest_pos = None;
                        let mut earliest_pp = None;
                        for pp in &self.pending_pastes {
                            if let Some(pos) = remaining.find(&pp.placeholder) {
                                if earliest_pos.map_or(true, |p| pos < p) {
                                    earliest_pos = Some(pos);
                                    earliest_pp = Some(pp);
                                }
                            }
                        }
                        match (earliest_pos, earliest_pp) {
                            (Some(pos), Some(pp)) => {
                                if pos > 0 {
                                    new_spans.push(Span::raw(remaining[..pos].to_string()));
                                }
                                new_spans.push(Span::styled(
                                    remaining[pos..pos + pp.placeholder.len()].to_string(),
                                    placeholder_style,
                                ));
                                remaining = &remaining[pos + pp.placeholder.len()..];
                            }
                            _ => {
                                if !remaining.is_empty() {
                                    new_spans.push(Span::raw(remaining.to_string()));
                                }
                                break;
                            }
                        }
                    }
                }
                line.spans = new_spans;
            }
        }

        (lines, cursor_row, cursor_col)
    }

    /// Render autocomplete suggestions as ratatui Lines (for display below the input box).
    pub fn render_autocomplete(&self, width: usize, max_lines: usize) -> Vec<Line<'static>> {
        let cyan = Style::default().fg(Color::Cyan);
        let dim = Style::default().add_modifier(Modifier::DIM);
        if !self.autocomplete_visible {
            return Vec::new();
        }
        let mut result = Vec::new();
        if let Some(ref suggestions) = self.autocomplete_suggestions {
            let total_items = suggestions.items.len();
            let max_items = std::cmp::min(total_items, 8);
            let count = std::cmp::min(max_items, max_lines);
            if count == 0 {
                return result;
            }
            let scroll_offset = if total_items > count && self.autocomplete_selection >= count {
                let offset = self.autocomplete_selection - count + 1;
                let max_offset = total_items - count;
                std::cmp::min(offset, max_offset)
            } else {
                0
            };
            let label_col = std::cmp::min(32, std::cmp::max(12, width.saturating_sub(20)));
            let desc_width = width.saturating_sub(label_col + 6);
            for i in scroll_offset..scroll_offset + count {
                if i >= total_items {
                    break;
                }
                let item = &suggestions.items[i];
                let selected = i == self.autocomplete_selection;
                let desc = item.description.as_deref().unwrap_or("");
                if selected {
                    let label_padded = format!("{:<width$}", item.label, width = label_col);
                    result.push(Line::from(vec![
                        Span::styled("→".to_string(), cyan),
                        Span::raw(" ".to_string()),
                        Span::styled(label_padded, cyan),
                        Span::raw(" ".to_string()),
                        Span::styled(desc.to_string(), cyan),
                    ]));
                } else {
                    let label_padded = format!("{:<width$}", item.label, width = label_col);
                    if desc_width > 4 {
                        let desc_truncated = if desc.len() > desc_width {
                            format!("{}…", &desc[..desc_width.saturating_sub(1)])
                        } else {
                            desc.to_string()
                        };
                        result.push(Line::from(vec![
                            Span::raw(format!("  {}", label_padded)),
                            Span::styled(format!(" {}", desc_truncated), dim),
                        ]));
                    } else {
                        result.push(Line::from(Span::raw(format!("  {}", label_padded))));
                    }
                }
            }
            if result.len() < max_lines && !suggestions.items.is_empty() {
                let current = self.autocomplete_selection + 1;
                result.push(Line::from(Span::styled(
                    format!("  ({}/{})", current, total_items),
                    dim,
                )));
            }
        }
        result
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_char() {
        let mut ed = Editor::new();
        ed.insert_char('a');
        ed.insert_char('b');
        ed.insert_char('c');
        assert_eq!(ed.buffer, "abc");
        assert_eq!(ed.cursor, 3);
    }

    #[test]
    fn test_insert_newline() {
        let mut ed = Editor::new();
        ed.insert_str("hello");
        ed.insert_newline();
        ed.insert_str("world");
        assert_eq!(ed.buffer, "hello\nworld");
        assert_eq!(ed.cursor_row_col(), (1, 5));
    }

    #[test]
    fn test_cursor_movement() {
        let mut ed = Editor::new();
        ed.insert_str("hello\nworld\nfoo");
        ed.cursor = 0;

        ed.cursor_down();
        assert_eq!(ed.cursor_row_col().0, 1);
        ed.cursor_right();
        assert_eq!(ed.cursor_row_col(), (1, 1));
        ed.cursor_up();
        assert_eq!(ed.cursor_row_col(), (0, 1));
    }

    #[test]
    fn test_cursor_line_start_end() {
        let mut ed = Editor::new();
        ed.insert_str("hello\nworld");
        ed.cursor = ed.buffer.len(); // after 'world'
        ed.cursor_line_start();
        let row = ed.cursor_row_col().0;
        assert_eq!(row, 1);
        assert_eq!(
            ed.buffer[..ed.cursor]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0),
            ed.cursor
        );
    }

    #[test]
    fn test_delete_before() {
        let mut ed = Editor::new();
        ed.insert_str("hello");
        ed.cursor = 5;
        ed.delete_before();
        assert_eq!(ed.buffer, "hell");
    }

    #[test]
    fn test_undo() {
        let mut ed = Editor::new();
        ed.insert_str("hello");
        ed.undo();
        assert_eq!(ed.buffer, "");
    }

    #[test]
    fn test_line_count() {
        let mut ed = Editor::new();
        assert_eq!(ed.line_count(), 1);
        ed.insert_str("hello\nworld\nfoo");
        assert_eq!(ed.line_count(), 3);
    }

    #[test]
    fn test_line_count_after_newline() {
        // Editor convention: each \n creates a new line
        let mut ed = Editor::new();
        ed.insert_str("hello");
        assert_eq!(ed.line_count(), 1);
        ed.insert_char('\n');
        // "hello\n" should be 2 lines: "hello" and ""
        assert_eq!(ed.line_count(), 2);
        assert_eq!(ed.buffer, "hello\n");
        assert_eq!(ed.cursor, 6);
    }

    #[test]
    fn test_backspace_deletes_text() {
        let mut ed = Editor::new();
        ed.insert_str("hello");
        assert_eq!(ed.buffer, "hello");
        assert_eq!(ed.cursor, 5);

        ed.delete_before();
        assert_eq!(ed.buffer, "hell");
        assert_eq!(ed.cursor, 4);

        ed.delete_before();
        assert_eq!(ed.buffer, "hel");
        assert_eq!(ed.cursor, 3);

        // Delete until empty
        ed.delete_before();
        ed.delete_before();
        ed.delete_before();
        assert_eq!(ed.buffer, "");
        assert_eq!(ed.cursor, 0);

        // Backspace at empty does nothing
        ed.delete_before();
        assert_eq!(ed.buffer, "");
        assert_eq!(ed.cursor, 0);
    }

    #[test]
    fn test_get_line() {
        let mut ed = Editor::new();
        ed.insert_str("line1\nline2\nline3");
        assert_eq!(ed.get_line(0), "line1");
        assert_eq!(ed.get_line(1), "line2");
        assert_eq!(ed.get_line(2), "line3");
    }

    #[test]
    fn test_word_movement() {
        let mut ed = Editor::new();
        ed.insert_str("hello world foo");
        ed.cursor = 0;
        ed.cursor_word_right();
        assert!(ed.cursor > 0);
        assert_eq!(&ed.buffer[..ed.cursor], "hello ");
    }

    /// Helper: extract plain text from a ratatui Line.
    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn test_chinese_characters() {
        let mut ed = Editor::new();
        ed.insert_char('你');
        ed.insert_char('好');
        assert_eq!(ed.buffer, "你好");
        assert_eq!(ed.cursor, 6, "cursor byte offset should be 6");

        let (lines, cursor_row, cursor_col) = ed.render(80, 5);
        assert!(!lines.is_empty(), "should produce at least one line");
        assert_eq!(
            line_text(&lines[0]),
            "你好",
            "line content should be '你好'"
        );
        assert_eq!(cursor_row, 0, "cursor row should be 0 for single line");
        assert_eq!(
            cursor_col, 4,
            "cursor col should be 4 (2 CJK chars × 2 width), not 6 (byte offset)"
        );
    }

    #[test]
    fn test_cursor_after_newline_on_empty_line() {
        let mut ed = Editor::new();
        ed.insert_str("abc");
        assert_eq!(ed.cursor, 3);
        ed.insert_newline_auto_indent();
        assert_eq!(ed.buffer, "abc\n", "buffer should end with newline");
        assert_eq!(ed.cursor, 4, "cursor should be after newline");

        let (lines, cursor_row, cursor_col) = ed.render(80, 10);
        assert!(
            lines.len() >= 2,
            "should have at least 2 lines after newline"
        );
        assert_eq!(cursor_row, 1, "cursor should be on the second line");
        assert_eq!(
            cursor_col, 0,
            "cursor should be at column 0 on the empty line"
        );
    }

    #[test]
    fn test_cursor_on_new_line_after_typing() {
        let mut ed = Editor::new();
        ed.insert_str("hello");
        ed.insert_newline();
        ed.insert_char('a');
        let (lines, cursor_row, cursor_col) = ed.render(80, 5);
        assert_eq!(lines.len(), 2, "should have 2 lines: {:?}", lines);
        assert_eq!(line_text(&lines[0]), "hello");
        assert_eq!(line_text(&lines[1]), "a");
        assert_eq!(cursor_row, 1, "cursor should be on second line (row 1)");
        assert_eq!(cursor_col, 1, "cursor should be after 'a' on second line");

        ed.insert_char('b');
        let (lines2, cursor_row2, cursor_col2) = ed.render(80, 5);
        assert_eq!(line_text(&lines2[1]), "ab");
        assert_eq!(cursor_row2, 1, "cursor stays on second line");
        assert_eq!(cursor_col2, 2, "cursor after 'ab'");
    }

    #[test]
    fn test_autocomplete_renders_on_slash() {
        let mut ed = Editor::new();
        let commands = vec![
            crate::autocomplete::SlashCommand {
                name: "settings".to_string(),
                description: Some("Open settings menu".to_string()),
                argument_hint: None,
            },
            crate::autocomplete::SlashCommand {
                name: "model".to_string(),
                description: Some("Select model".to_string()),
                argument_hint: None,
            },
        ];
        ed.set_autocomplete_provider(Box::new(
            crate::autocomplete::CombinedAutocompleteProvider::new(
                commands,
                std::path::PathBuf::from("/tmp"),
            ),
        ));

        ed.insert_char('/');
        assert!(
            ed.is_autocomplete_active(),
            "autocomplete should activate after typing /"
        );

        let (lines, _cursor_row, _cursor_col) = ed.render(80, 10);
        assert_eq!(
            lines.len(),
            1,
            "render() should only contain text line, got: {:?}",
            lines
        );
        let text = line_text(&lines[0]);
        assert!(text.contains("/"), "first line should show the input text");

        let ac_lines = ed.render_autocomplete(80, 10);
        assert!(
            ac_lines.len() >= 3,
            "should have 2 suggestions + counter, got {}: {:?}",
            ac_lines.len(),
            ac_lines
        );
        let t0 = line_text(&ac_lines[0]);
        assert!(
            t0.contains("→"),
            "first item should be selected with → marker, got: {:?}",
            t0
        );
        assert!(
            t0.contains("settings"),
            "first line should show settings, got: {:?}",
            t0
        );
        let t1 = line_text(&ac_lines[1]);
        assert!(
            t1.contains("model"),
            "second line should show model, got: {:?}",
            t1
        );
        let last = line_text(ac_lines.last().unwrap());
        assert!(
            last.contains("(1/2)"),
            "counter should show (1/2), got: {:?}",
            last
        );
    }

    #[test]
    fn test_clear() {
        let mut ed = Editor::new();
        ed.insert_str("hello");
        ed.clear();
        assert!(ed.buffer.is_empty());
        assert_eq!(ed.cursor, 0);
    }
}
