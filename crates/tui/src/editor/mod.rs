//! Multi-line text editor with Emacs-style keybindings

use unicode_width::UnicodeWidthStr;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use pick_agent::prompt_history::HistoryProvider;

use crate::autocomplete::{AutocompleteProvider, AutocompleteSuggestions};
use crate::kill_ring::KillRing;
use crate::undo_stack::UndoStack;

mod types;
pub use types::*;

/// Byte range of a paste placeholder element in the editor buffer.
/// The element is treated as an atomic unit: cursor skips over it,
/// Backspace/Delete removes the entire element at once.
#[derive(Debug, Clone)]
pub struct PasteElement {
    pub start: usize,
    pub end: usize,
}

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

    // --- Autocomplete ---
    /// Optional autocomplete provider for slash commands and file paths
    autocomplete_provider: Option<Box<dyn AutocompleteProvider>>,
    /// Current autocomplete suggestions (None when not active)
    autocomplete_suggestions: Option<AutocompleteSuggestions>,
    /// Currently selected suggestion index
    autocomplete_selection: usize,
    /// Whether to show the autocomplete popup
    autocomplete_visible: bool,

    // --- Prompt history (delegated to HistoryProvider) ---
    /// Optional history provider for input history navigation.
    pub history: Option<Box<dyn HistoryProvider>>,
    /// Whether the user is currently browsing history entries.
    browsing_history: bool,

    // --- Pending messages browsing ---
    /// Index into pending_user_messages when browsing (None = not browsing)
    pub pending_index: Option<usize>,
    /// Saved current input buffer when entering pending-message browsing
    pending_staging: String,

    // --- Paste placeholder ---
    /// Maps a placeholder string (displayed in the buffer) to the actual
    /// pasted content (used on submit to expand placeholders).
    /// When `paste_elements` tracking detects deletion of a placeholder,
    /// the corresponding entry is removed from this list.
    pending_pastes: Vec<(String, String)>,
    /// Byte ranges of paste placeholder elements in `buffer`.
    /// These are kept aligned via `shift_elements_after` and
    /// `validate_paste_elements`.
    paste_elements: Vec<PasteElement>,
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

            autocomplete_provider: None,
            autocomplete_suggestions: None,
            autocomplete_selection: 0,
            autocomplete_visible: false,
            history: None,
            browsing_history: false,
            pending_index: None,
            pending_staging: String::new(),
            pending_pastes: Vec::new(),
            paste_elements: Vec::new(),
        }
    }

    /// Push current state onto undo stack
    fn push_undo(&mut self) {
        self.undo_stack.push(&self.buffer.clone());
    }

    /// Shift all paste-element byte ranges that start at or after `at`
    /// by `delta` bytes (positive for insert, negative for delete).
    fn shift_elements_after(&mut self, at: usize, delta: isize) {
        for elem in &mut self.paste_elements {
            if elem.start >= at {
                elem.start = (elem.start as isize + delta) as usize;
                elem.end = (elem.end as isize + delta) as usize;
            }
        }
    }

    /// Remove any paste elements whose byte range is invalid or whose
    /// placeholder text no longer matches the buffer content.
    fn validate_paste_elements(&mut self) {
        let mut i = 0;
        while i < self.paste_elements.len() {
            let e = &self.paste_elements[i];
            if e.end > self.buffer.len() || e.start >= e.end {
                self.paste_elements.swap_remove(i);
                continue;
            }
            let placeholder = &self.buffer[e.start..e.end];
            let valid = self.pending_pastes.iter().any(|(ph, _)| ph == placeholder);
            if !valid {
                self.paste_elements.swap_remove(i);
                continue;
            }
            i += 1;
        }
    }

    // --- Insertion ---

    /// Insert a character at cursor position
    pub fn insert_char(&mut self, c: char) {
        self.push_undo();
        // Any manual edit exits history browsing
        self.reset_history();
        self.buffer.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.mark = None;
        self.kill_accumulating = false;
        self.shift_elements_after(self.cursor - c.len_utf8(), c.len_utf8() as isize);
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
        self.insert_str_impl(s, false);
    }

    /// Insert a string without merging into any adjacent paste element.
    /// Used for non-paste text accumulated during normal typing, ensuring
    /// typed characters appear visibly in the buffer instead of being
    /// swallowed into a paste element's pending_pastes.
    pub fn insert_str_no_merge(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.push_undo();
        self.reset_history();
        self.mark = None;
        self.kill_accumulating = false;
        self.buffer.insert_str(self.cursor, s);
        self.cursor += s.len();
        self.shift_elements_after(self.cursor - s.len(), s.len() as isize);
    }

    /// Insert a string during a confirmed paste burst.
    /// Always creates/merges a PasteElement regardless of content size,
    /// so the first batch of a multi-batch paste creates a placeholder
    /// even when the individual batch is below the 100-char / 11-line threshold.
    pub fn insert_str_paste_burst(&mut self, s: &str) {
        self.insert_str_impl(s, true);
    }

    /// Internal implementation shared by `insert_str` and `insert_str_paste_burst`.
    /// When `force_element` is true, the size threshold is bypassed and a
    /// PasteElement is always created (or merged with an existing one at cursor).
    fn insert_str_impl(&mut self, s: &str, force_element: bool) {
        if s.is_empty() {
            return;
        }
        self.reset_history();
        self.mark = None;
        self.kill_accumulating = false;

        let line_count = s.lines().count();
        if s.len() > 100 || line_count > 11 || force_element {
            // Check if cursor is at the end of an existing paste element.
            // If so, merge the new content into it (handles pastes that
            // arrive in multiple batches due to timing / platform quirks).
            if let Some(idx) = self
                .paste_elements
                .iter()
                .position(|e| e.end == self.cursor)
            {
                let old_ph = self.buffer
                    [self.paste_elements[idx].start..self.paste_elements[idx].end]
                    .to_string();
                if let Some(pp_idx) = self.pending_pastes.iter().position(|(ph, _)| *ph == old_ph) {
                    let combined = self.pending_pastes[pp_idx].1.clone() + s;
                    let new_line_count = combined.lines().count();
                    let new_ph = format!("[Pasted Content {} Lines]", new_line_count);
                    let new_ph_len = new_ph.len();
                    let start = self.paste_elements[idx].start;
                    let old_len = self.paste_elements[idx].end - start;

                    self.push_undo();
                    // Replace the old placeholder text with the new one
                    self.buffer.drain(start..self.paste_elements[idx].end);
                    self.buffer.insert_str(start, &new_ph);
                    self.cursor = start + new_ph_len;
                    // Update element range
                    let delta = new_ph_len as isize - old_len as isize;
                    self.paste_elements[idx].end = start + new_ph_len;
                    // Update pending paste entry
                    self.pending_pastes[pp_idx] = (new_ph, combined);
                    // Shift subsequent elements by the delta
                    self.shift_elements_after(start + old_len, delta);
                    return;
                }
            }

            // No existing element at cursor — create a new one.
            let placeholder = format!("[Pasted Content {} Lines]", line_count);
            let ph_len = placeholder.len();
            self.push_undo();
            let at = self.cursor;
            self.buffer.insert_str(at, &placeholder);
            self.cursor = at + ph_len;
            self.shift_elements_after(at, ph_len as isize);
            self.paste_elements.push(PasteElement {
                start: at,
                end: at + ph_len,
            });
            self.pending_pastes.push((placeholder, s.to_string()));
        } else {
            // Small insertion (<100 chars, <=11 lines).
            // If cursor is at the end of a paste element, merge this text
            // into the existing placeholder to handle paste batches that
            // arrive below the threshold.
            if let Some(idx) = self
                .paste_elements
                .iter()
                .position(|e| e.end == self.cursor)
            {
                let old_ph = self.buffer
                    [self.paste_elements[idx].start..self.paste_elements[idx].end]
                    .to_string();
                if let Some(pp_idx) = self.pending_pastes.iter().position(|(ph, _)| *ph == old_ph) {
                    let combined = self.pending_pastes[pp_idx].1.clone() + s;
                    let new_line_count = combined.lines().count();
                    let new_ph = format!("[Pasted Content {} Lines]", new_line_count);
                    let new_ph_len = new_ph.len();
                    let start = self.paste_elements[idx].start;
                    let old_len = self.paste_elements[idx].end - start;

                    self.push_undo();
                    self.buffer.drain(start..self.paste_elements[idx].end);
                    self.buffer.insert_str(start, &new_ph);
                    self.cursor = start + new_ph_len;
                    let delta = new_ph_len as isize - old_len as isize;
                    self.paste_elements[idx].end = start + new_ph_len;
                    self.pending_pastes[pp_idx] = (new_ph, combined);
                    self.shift_elements_after(start + old_len, delta);
                    return;
                }
            }

            self.push_undo();
            self.buffer.insert_str(self.cursor, s);
            self.cursor += s.len();
            self.shift_elements_after(self.cursor - s.len(), s.len() as isize);
        }
    }

    /// Insert a newline at cursor position
    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    /// Insert auto-indent (newline + matching indent)
    pub fn insert_newline_auto_indent(&mut self) {
        self.push_undo();
        // Any manual edit exits history browsing
        self.reset_history();
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

        let at = self.cursor;
        self.buffer.insert(self.cursor, '\n');
        self.cursor += 1;
        self.buffer.insert_str(self.cursor, &indent);
        self.cursor += indent.len();
        self.shift_elements_after(at, (1 + indent.len()) as isize);
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
        // Find the exclusive end of the target line — either the \n position
        // (always a char boundary since \n is ASCII) or buffer.len() for the last line
        let next_line_end = self.buffer[next_line_start..]
            .find('\n')
            .map(|i| next_line_start + i) // position of \n = exclusive end
            .unwrap_or(self.buffer.len()); // end of buffer = exclusive end
        self.cursor = self.offset_from_visual_col(next_line_start, next_line_end, target_col);
        self.mark = None;
        self.kill_accumulating = false;
    }

    /// Move cursor left by one character (skipping paste elements)
    pub fn cursor_left(&mut self) {
        if self.cursor > 0 {
            // If at end of an element, jump to its start
            if let Some(elem) = self.paste_elements.iter().find(|e| e.end == self.cursor) {
                self.cursor = elem.start;
            } else {
                self.cursor = self.cursor.saturating_sub(1);
                while !self.buffer.is_char_boundary(self.cursor) {
                    self.cursor = self.cursor.saturating_sub(1);
                }
                // If we landed inside an element, snap to its start
                if let Some(elem) = self
                    .paste_elements
                    .iter()
                    .find(|e| e.start < self.cursor && self.cursor < e.end)
                {
                    self.cursor = elem.start;
                }
            }
        }
        self.mark = None;
        self.kill_accumulating = false;
    }

    /// Move cursor right by one character (skipping paste elements)
    pub fn cursor_right(&mut self) {
        if self.cursor < self.buffer.len() {
            // If at start of an element, jump to its end
            if let Some(elem) = self.paste_elements.iter().find(|e| e.start == self.cursor) {
                self.cursor = elem.end;
            } else {
                self.cursor += 1;
                while self.cursor < self.buffer.len() && !self.buffer.is_char_boundary(self.cursor)
                {
                    self.cursor += 1;
                }
                // If we landed inside an element, snap to its end
                if let Some(elem) = self
                    .paste_elements
                    .iter()
                    .find(|e| e.start < self.cursor && self.cursor < e.end)
                {
                    self.cursor = elem.end;
                }
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
                .is_some_and(|c| c.is_alphanumeric() || c == '_')
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
                .is_some_and(|c| c.is_alphanumeric() || c == '_')
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
        self.reset_history();
        // If cursor is at the end of a paste element, delete the entire
        // element atomically (remove placeholder from buffer + tracking).
        if let Some(idx) = self
            .paste_elements
            .iter()
            .position(|e| e.end == self.cursor)
        {
            self.push_undo();
            let start = self.paste_elements[idx].start;
            let end = self.paste_elements[idx].end;
            let placeholder = self.buffer[start..end].to_string();
            self.buffer.drain(start..end);
            self.cursor = start;
            self.pending_pastes.retain(|(ph, _)| *ph != placeholder);
            self.paste_elements.swap_remove(idx);
            self.shift_elements_after(start, -((end - start) as isize));
            self.kill_accumulating = false;
            return;
        }
        // If there's a selection, delete it
        if self.mark.is_some() {
            self.delete_selection();
            return;
        }
        self.push_undo();
        let prev = self.prev_char_boundary(self.cursor);
        let removed_len = self.cursor - prev;
        let deleted = self.buffer[prev..self.cursor].to_string();
        self.buffer.drain(prev..self.cursor);
        self.cursor = prev;
        self.shift_elements_after(prev, -(removed_len as isize));
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
        self.reset_history();
        // If cursor is at the start of a paste element, delete the entire
        // element atomically.
        if let Some(idx) = self
            .paste_elements
            .iter()
            .position(|e| e.start == self.cursor)
        {
            self.push_undo();
            let start = self.paste_elements[idx].start;
            let end = self.paste_elements[idx].end;
            let placeholder = self.buffer[start..end].to_string();
            self.buffer.drain(start..end);
            self.cursor = start;
            self.pending_pastes.retain(|(ph, _)| *ph != placeholder);
            self.paste_elements.swap_remove(idx);
            self.shift_elements_after(start, -((end - start) as isize));
            self.kill_accumulating = false;
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
        self.shift_elements_after(self.cursor, -((next - self.cursor) as isize));
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
        self.shift_elements_after(word_start, -(deleted.len() as isize));
        self.validate_paste_elements();
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
        self.shift_elements_after(self.cursor, -(deleted.len() as isize));
        self.validate_paste_elements();
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
        self.shift_elements_after(line_start, -(deleted.len() as isize));
        self.validate_paste_elements();
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
        self.shift_elements_after(self.cursor, -(deleted.len() as isize));
        self.validate_paste_elements();
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
        self.shift_elements_after(line_start, -(deleted.len() as isize));
        self.validate_paste_elements();
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
        let len = end - start;
        self.buffer.drain(start..end);
        self.cursor = start;
        self.shift_elements_after(start, -(len as isize));
        self.validate_paste_elements();
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
            self.validate_paste_elements();
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
        let at = self.cursor;
        self.buffer.insert_str(self.cursor, &text);
        // Position cursor after the yanked text, set mark at start
        let mark_pos = at;
        self.cursor += text.len();
        self.shift_elements_after(at, text.len() as isize);
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
        self.shift_elements_after(start, -(yank_size as isize));
        // Rotate kill ring and insert next entry
        self.kill_ring.rotate();
        match self.kill_ring.peek() {
            Some(t) => {
                let at = self.cursor;
                let t_len = t.len();
                self.buffer.insert_str(self.cursor, t);
                self.mark = Some(self.cursor);
                self.cursor += t_len;
                self.shift_elements_after(at, t_len as isize);
                self.last_yank_size = Some(t_len);
            }
            None => {
                self.mark = None;
                self.last_yank_size = None;
            }
        }
        self.validate_paste_elements();
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
            self.validate_paste_elements();
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
            self.validate_paste_elements();
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
        if let Some(ref suggestions) = self.autocomplete_suggestions
            && suggestions.items.len() > 1
        {
            self.autocomplete_selection =
                (self.autocomplete_selection + 1) % suggestions.items.len();
        }
    }

    /// Cycle autocomplete selection to the previous item
    pub fn autocomplete_previous(&mut self) {
        if let Some(ref suggestions) = self.autocomplete_suggestions
            && suggestions.items.len() > 1
        {
            self.autocomplete_selection = if self.autocomplete_selection == 0 {
                suggestions.items.len() - 1
            } else {
                self.autocomplete_selection - 1
            };
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
        self.pending_pastes.clear();
        self.paste_elements.clear();
    }

    /// Set the buffer content and reset cursor
    pub fn set_text(&mut self, text: &str) {
        self.push_undo();
        self.buffer = text.to_string();
        self.cursor = self.buffer.len();
        self.mark = None;
        self.scroll_offset = 0;
        self.kill_accumulating = false;
        self.pending_pastes.clear();
        self.paste_elements.clear();
    }

    /// Reset history browsing state (called on any manual edit)
    fn reset_history(&mut self) {
        self.browsing_history = false;
        if let Some(h) = &mut self.history {
            h.reset();
        }
    }

    /// Set the history provider for input history navigation.
    pub fn set_history_provider(&mut self, provider: Box<dyn HistoryProvider>) {
        self.history = Some(provider);
    }

    /// Get the buffer content
    pub fn text(&self) -> &str {
        &self.buffer
    }

    /// Whether the editor has any active paste elements (placeholders
    /// inserted for long pastes that haven't been submitted yet).
    pub fn has_paste_elements(&self) -> bool {
        !self.paste_elements.is_empty()
    }

    /// Get the full text with all paste placeholders expanded to their
    /// actual content.  This is what should be submitted to the agent.
    pub fn full_text(&self) -> String {
        let mut text = self.buffer.clone();
        for (placeholder, actual) in &self.pending_pastes {
            text = text.replace(placeholder.as_str(), actual.as_str());
        }
        text
    }

    // --- Prompt history (delegated to HistoryProvider) ---

    /// Returns `true` when the user is currently navigating history.
    pub fn is_browsing_history(&self) -> bool {
        self.browsing_history
    }

    /// Push a submitted text into input history (delegates to provider).
    pub fn push_history(&mut self, text: String) {
        if let Some(h) = &mut self.history {
            h.push(&text);
        }
        self.browsing_history = false;
    }

    /// Navigate backward (older) in input history.
    /// Returns `true` if navigation occurred.
    pub fn history_previous(&mut self) -> bool {
        let text = self.history.as_mut().and_then(|h| h.previous(&self.buffer));
        if let Some(t) = text {
            self.set_text(&t);
            self.browsing_history = true;
            true
        } else {
            false
        }
    }

    /// Navigate forward (newer) in input history.
    /// Returns `true` if navigation occurred.
    pub fn history_next(&mut self) -> bool {
        let result = self.history.as_mut().and_then(|h| h.next(&self.buffer));
        if let Some(ref t) = result {
            self.set_text(t);
            // Check if provider exited browse mode
            let still_browsing = self.history.as_ref().is_some_and(|h| h.is_browsing());
            self.browsing_history = still_browsing;
            true
        } else {
            false
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
                self.pending_staging = self.buffer.clone();
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
            if self.pending_staging.is_empty() {
                self.clear();
            } else {
                let text = self.pending_staging.clone();
                self.pending_staging.clear();
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
        let mut found = false;
        for (i, c) in self.buffer.char_indices() {
            if line_starts == row {
                line_start_offset = i;
                found = true;
                break;
            }
            if c == '\n' {
                line_starts += 1;
            }
        }
        // Exhausted the buffer without finding the line start:
        // the cursor must be on the last (empty) line.
        if !found && line_starts == row {
            line_start_offset = self.buffer.len();
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

    /// Auto-adjust scroll_offset to keep the cursor in the visible area.
    /// Called before render so the cursor line is always shown.
    pub fn ensure_cursor_visible(&mut self, width: usize, max_height: usize) {
        if max_height == 0 {
            return;
        }
        // Count visual lines before the cursor
        let mut visual_before_cursor = 0usize;
        let mut pos = 0;
        let cursor_line_start = self.buffer[..self.cursor]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        loop {
            if pos >= cursor_line_start {
                break;
            }
            let nl = self.buffer[pos..]
                .find('\n')
                .map(|i| pos + i)
                .unwrap_or(self.buffer.len());
            let line = &self.buffer[pos..nl];
            if width > 0 {
                visual_before_cursor += wrap_by_display_width(line, width).len();
            } else {
                visual_before_cursor += 1;
            }
            pos = nl + 1;
        }
        // Cursor is within the cursor's logical line; count its visual row within that line
        let col_in_line = self.cursor - cursor_line_start;
        let cursor_line_text = &self.buffer[cursor_line_start..];
        let prefix = &cursor_line_text[..col_in_line];
        let prefix_visual_lines = if width > 0 && prefix.width() > width {
            wrap_by_display_width(prefix, width).len().saturating_sub(1)
        } else {
            0
        };
        let cursor_visual_line = visual_before_cursor + prefix_visual_lines;

        // Adjust scroll_offset so cursor is visible
        if cursor_visual_line < self.scroll_offset {
            self.scroll_offset = cursor_visual_line;
        } else if cursor_visual_line >= self.scroll_offset + max_height {
            self.scroll_offset = cursor_visual_line.saturating_sub(max_height.saturating_sub(1));
        }

        // Clamp so scroll_offset never exceeds the last content line.
        // `visual_line_count` does NOT count a trailing empty line after
        // `\n`, but `cursor_visual_line` CAN land on it (e.g. after
        // Shift+Enter inserts `\n` and cursor moves past it).  Without
        // clamping, the unadjusted scroll_offset pushes all content past
        // `visible_top`, making the editor appear empty even though the
        // buffer still holds the text.
        let total_visual = self.visual_line_count(width);
        let max_scroll = total_visual.saturating_sub(max_height);
        self.scroll_offset = self.scroll_offset.min(max_scroll);
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
        // Trailing \n adds one empty line (the cursor lands here after
        // Shift+Enter, and the editor needs at least 2 lines to show
        // both the content and the trailing blank line).
        if self.buffer.ends_with('\n') {
            count + 1
        } else {
            count
        }
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
                    let last_empty = lines.last().is_none_or(|l| {
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
            // Cursor line wasn't rendered: clamp to whichever end of the visible
            // output the cursor is on, so the user sees the cursor near where
            // they are editing rather than jumping to (0,0).
            if lines.is_empty() {
                cursor_row = 0;
                cursor_col = 0;
            } else if self.cursor < cursor_line_start {
                // Cursor is above the visible area
                cursor_row = 0;
                cursor_col = 0;
            } else {
                // Cursor is below the visible area (clipped by max_height)
                cursor_row = lines.len().saturating_sub(1);
                cursor_col = 0;
            }
        }

        // Trim trailing empty lines
        if !self.buffer.ends_with('\n') {
            while lines.len() > 1 {
                let is_empty = lines.last().is_none_or(|l| {
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

        // Highlight paste placeholder elements in cyan.
        if !self.paste_elements.is_empty() {
            let cyan = Style::default().fg(Color::Cyan);
            for line in &mut lines {
                let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                for (ph, _) in &self.pending_pastes {
                    if let Some(pos) = line_text.find(ph.as_str()) {
                        let before = &line_text[..pos];
                        let after = &line_text[pos + ph.len()..];
                        let mut new_spans: Vec<Span<'static>> = Vec::new();
                        if !before.is_empty() {
                            new_spans.push(Span::raw(before.to_string()));
                        }
                        new_spans.push(Span::styled(ph.clone(), cyan));
                        if !after.is_empty() {
                            new_spans.push(Span::raw(after.to_string()));
                        }
                        line.spans = new_spans;
                        break; // only one placeholder per line
                    }
                }
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
            let label_col = width.saturating_sub(20).clamp(12, 32);
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

    // --- Cursor movement edge case tests ---

    #[test]
    fn test_cursor_up_down_chinese_text() {
        // User scenario: paste Chinese text with multiple lines
        let mut ed = Editor::new();
        ed.insert_str("magegen — 生成或编辑位图图片\n3. hv-analysis — 横纵分析法");
        ed.cursor = 0;

        // Move down to line 2
        ed.cursor_down();
        assert_eq!(
            ed.cursor_row_col().0,
            1,
            "cursor_down should move to line 2 (row=1)",
        );

        // Move up back to line 1
        ed.cursor_up();
        assert_eq!(
            ed.cursor_row_col().0,
            0,
            "cursor_up from line 2 should move to line 1 (row=0)",
        );
    }

    #[test]
    fn test_cursor_up_down_with_trailing_newline() {
        // Buffer ends with \n — the last line is empty
        let mut ed = Editor::new();
        ed.insert_str("hello\nworld\n");
        ed.cursor = ed.buffer.len(); // on the empty last line

        // visual_col_at_line on last empty line
        let (row, _col) = ed.cursor_row_col();
        assert!(row > 0, "cursor should be on a non-first row");

        ed.cursor_up();
        // Should have moved to line 2 (row=1)
        assert_eq!(
            ed.cursor_row_col().0,
            1,
            "cursor_up from (trailing) empty last line should move to row=1",
        );
    }

    #[test]
    fn test_cursor_down_from_last_line_does_not_wrap() {
        // Pressing down on last line should stay at end
        let mut ed = Editor::new();
        ed.insert_str("line1\nline2\nline3");
        ed.cursor = ed.buffer.len(); // end of last line

        ed.cursor_down();
        assert_eq!(
            ed.cursor,
            ed.buffer.len(),
            "cursor_down on last line should stay at end",
        );

        // Second press should also stay at end
        ed.cursor_down();
        assert_eq!(
            ed.cursor,
            ed.buffer.len(),
            "cursor_down on last line (2nd press) should stay at end",
        );
    }

    #[test]
    fn test_cursor_up_from_first_line_stays_at_start() {
        let mut ed = Editor::new();
        ed.insert_str("multi\nline\ntext");

        // cursor_up from first line start
        ed.cursor = 0;
        ed.cursor_up();
        assert_eq!(ed.cursor, 0, "cursor_up from line 1 start should stay at 0",);

        // cursor_up from middle of first line
        ed.cursor = 3;
        ed.cursor_up();
        assert_eq!(
            ed.cursor, 0,
            "cursor_up from middle of line 1 should go to 0",
        );
    }

    #[test]
    fn test_cursor_up_down_multi_line_paste_scenario() {
        // Simulate pasting text that ends with \n (common clipboard behavior)
        let mut ed = Editor::new();
        ed.insert_str("aa\nbb\ncc\n");

        // Cursor is at end (after trailing \n, on empty last line)
        assert_eq!(
            ed.cursor_row_col().0,
            3,
            "cursor should be on row 3 (0-indexed, 4th line)",
        );

        // Move up to line 3
        ed.cursor_up();
        assert_eq!(ed.cursor_row_col().0, 2, "should be on row 2 (line 'cc')");

        // Move up to line 2
        ed.cursor_up();
        assert_eq!(ed.cursor_row_col().0, 1, "should be on row 1 (line 'bb')");

        // Move up to line 1
        ed.cursor_up();
        assert_eq!(ed.cursor_row_col().0, 0, "should be on row 0 (line 'aa')");

        // Move up again — stays on line 1
        ed.cursor_up();
        assert_eq!(
            ed.cursor_row_col().0,
            0,
            "should stay on row 0 after extra cursor_up",
        );
    }

    #[test]
    fn test_cursor_preserves_visual_column_across_lines() {
        let mut ed = Editor::new();
        ed.insert_str("abc\ndefg\nh");

        // Place cursor at column 3 on line 2 ('g' in "defg")
        ed.cursor = 6; // position of 'g': "abc\n"=4, "def"=5, "g"=6
        let (row, col) = ed.cursor_row_col();
        assert_eq!(row, 1, "cursor should be on row 1 (0-indexed), got {row}",);
        assert_eq!(
            col, 2,
            "cursor should be byte-offset 2 in line 2, got {col}"
        );

        // Move up to line 1 — cursor should move to 'c' (col 2, same visual column)
        ed.cursor_up();
        let (new_row, _new_col) = ed.cursor_row_col();
        assert_eq!(new_row, 0, "cursor_up should move to row 0, got {new_row}",);

        // Move down back to line 2
        ed.cursor_down();
        let (down_row, down_col) = ed.cursor_row_col();
        assert_eq!(
            down_row, 1,
            "cursor_down should return to row 1, got {down_row}",
        );
        assert_eq!(
            down_col, 2,
            "cursor_down should preserve visual col, got byte offset {down_col}",
        );
    }

    #[test]
    fn test_visual_col_at_line_after_trailing_newline() {
        // Directly test visual_col_at_line via cursor movement position
        let mut ed = Editor::new();
        ed.insert_str("ab\nc\n");
        // Bytes: a=0, b=1, \n=2, c=3, \n=4
        // Lines: 0="ab", 1="c", 2=""

        // Place cursor at end of empty last line
        ed.cursor = ed.buffer.len(); // position 5 (past all content)

        // Move up to line 2
        ed.cursor_up();
        assert_eq!(ed.cursor_row_col().0, 1, "should move to row 1 (line 'c')");

        // Move up to line 1
        ed.cursor_up();
        assert_eq!(ed.cursor_row_col().0, 0, "should move to row 0 (line 'ab')");
    }

    #[test]
    fn test_cursor_down_then_up_sequence() {
        // Comprehensive sequence: up/down across all lines
        let mut ed = Editor::new();
        ed.insert_str("alpha\nbeta\n gamma \ndelta");

        // Start at beginning
        ed.cursor = 0;

        // Down to line 2
        ed.cursor_down();
        assert_eq!(ed.cursor_row_col().0, 1, "down → row 1");
        // Down to line 3
        ed.cursor_down();
        assert_eq!(ed.cursor_row_col().0, 2, "down → row 2");
        // Down to line 4
        ed.cursor_down();
        assert_eq!(ed.cursor_row_col().0, 3, "down → row 3");
        // Down again — should stay on line 4
        ed.cursor_down();
        assert_eq!(ed.cursor_row_col().0, 3, "down at end → still row 3");

        // Up to line 3
        ed.cursor_up();
        assert_eq!(ed.cursor_row_col().0, 2, "up → row 2");
        // Up to line 2
        ed.cursor_up();
        assert_eq!(ed.cursor_row_col().0, 1, "up → row 1");
        // Up to line 1
        ed.cursor_up();
        assert_eq!(ed.cursor_row_col().0, 0, "up → row 0");
        // Up again — should stay on line 1
        ed.cursor_up();
        assert_eq!(ed.cursor_row_col().0, 0, "up at top → still row 0");
    }

    #[test]
    fn test_cursor_movement_with_fullwidth_chars() {
        // Fullwidth Chinese and emoji
        let mut ed = Editor::new();
        ed.insert_str("ＡＢ\nＣ\nＤＥＦ");

        ed.cursor = 0;
        // Move down through all lines
        ed.cursor_down();
        assert_eq!(ed.cursor_row_col().0, 1, "fullwidth: down → row 1");
        ed.cursor_down();
        assert_eq!(ed.cursor_row_col().0, 2, "fullwidth: down → row 2");
        ed.cursor_down();
        assert_eq!(
            ed.cursor_row_col().0,
            2,
            "fullwidth: down at end → still row 2",
        );

        // Move up back to start
        ed.cursor_up();
        assert_eq!(ed.cursor_row_col().0, 1, "fullwidth: up → row 1");
        ed.cursor_up();
        assert_eq!(ed.cursor_row_col().0, 0, "fullwidth: up → row 0");
        ed.cursor_up();
        assert_eq!(
            ed.cursor_row_col().0,
            0,
            "fullwidth: up at top → still row 0",
        );
    }

    #[test]
    fn test_insert_newline_auto_indent_creates_new_line() {
        let mut ed = Editor::new();
        ed.insert_str("hello");
        assert_eq!(ed.buffer, "hello");
        assert_eq!(ed.cursor, 5);

        // Ctrl+Enter inserts a newline and places cursor on the new line
        ed.insert_newline_auto_indent();
        assert_eq!(ed.buffer, "hello\n", "Ctrl+Enter should insert newline");
        assert_eq!(
            ed.cursor, 6,
            "cursor should move past the newline, got {}",
            ed.cursor,
        );
        let (row, col) = ed.cursor_row_col();
        assert_eq!(
            row, 1,
            "cursor should be on line 2 (row=1), got row={}",
            row,
        );
        assert_eq!(
            col, 0,
            "cursor should be at column 0 of the new line, got col={}",
            col,
        );

        // Typing a character should work on the new line
        ed.insert_char('x');
        assert_eq!(
            ed.buffer, "hello\nx",
            "typing after Ctrl+Enter should insert on new line",
        );
        assert_eq!(ed.cursor, 7, "cursor should be after 'x'");
    }

    #[test]
    fn test_ctrl_enter_render_shows_cursor_on_new_line() {
        // Simulate: type "hello", Ctrl+Enter, then render and check cursor position
        let mut ed = Editor::new();
        ed.show_prompt = true;
        ed.prompt = "❯ ".to_string();
        ed.insert_str("hello");
        ed.insert_newline_auto_indent();

        // Render with a height big enough to fit both lines
        let (lines, cursor_row, cursor_col) = ed.render(80, 10);
        assert_eq!(
            lines.len(),
            2,
            "render should output 2 lines (prompt + empty)"
        );
        assert_eq!(
            cursor_row, 1,
            "cursor should be on rendered line 1 (0-indexed), got {cursor_row}",
        );
        assert_eq!(
            cursor_col, 0,
            "cursor col should be 0 on the empty new line, got {cursor_col}",
        );
    }

    #[test]
    fn test_ctrl_enter_on_nonempty_line_with_content_after() {
        let mut ed = Editor::new();
        ed.insert_str("abc\ndef");
        // Cursor at end of buffer (position 7 = after 'f')
        ed.cursor = ed.buffer.len();
        ed.insert_newline_auto_indent();

        assert_eq!(
            ed.buffer, "abc\ndef\n",
            "Ctrl+Enter at end should append newline"
        );
        let (row, col) = ed.cursor_row_col();
        assert_eq!(
            row, 2,
            "cursor should be on the new blank line (row=2, col=0), got row={row}",
        );
        assert_eq!(col, 0, "cursor col should be 0 on the new line, got {col}",);
    }
}
