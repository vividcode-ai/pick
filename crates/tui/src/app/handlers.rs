//! TUI event handling functions

use std::io::Write;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use crate::autocomplete::AutocompleteProvider;
use crate::components::select::SelectList;
use crate::editor::Editor;

use super::types::AppState;
use super::types::TreeView;
use super::types::TuiAction;
use super::types::TuiApp;
use super::types::UpdateChoice;

impl TuiApp {
    /// Create a new TUI app (enables raw mode for keyboard input)
    pub fn new(
        provider: &str,
        model_id: &str,
        app_name: &str,
        version: &str,
        context_file_names: Vec<String>,
        skill_names: Vec<String>,
        cwd: &str,
        home_dir: Option<String>,
        thinking_level: &str,
        autocomplete_provider: Option<Box<dyn AutocompleteProvider>>,
        folder: &str,
        agent_mode: &str,
    ) -> Result<Self, String> {
        enable_raw_mode().map_err(|e| format!("Failed to enable raw mode: {}", e))?;

        let mut editor = Editor::new();
        editor.show_prompt = true;
        editor.prompt = "\u{276f} ".to_string();
        if let Some(provider) = autocomplete_provider {
            editor.set_autocomplete_provider(provider);
        }

        let chat = crate::components::chat::ChatView::new();
        Ok(Self {
            chat_lines_written: 0,
            confirm_quit: false,
            chat,
            editor,
            state: AppState::Input,
            provider: provider.to_string(),
            model_id: model_id.to_string(),
            app_name: app_name.to_string(),
            version: version.to_string(),
            startup_header_added: false,
            context_file_names,
            skill_names,
            cwd: cwd.to_string(),
            home_dir,
            total_input: 0,
            total_output: 0,
            total_cache_read: 0,
            total_cache_write: 0,
            context_percent: Some(0.0),
            context_window: 1_000_000,
            git_branch: None,
            session_name: None,
            thinking_level: thinking_level.to_string(),
            auto_compact: true,
            selection: None,
            tree_view: None,
            api_key_provider: None,
            api_key_input: String::new(),
            agent_mode: agent_mode.to_string(),
            cached_lines_entry_count: 0,
            cached_lines_committed: 0,
            last_render_width: 0,
            autocomplete_space_lines: 0,
            status_text: None,
            status_frame: 0,
            agent_start_time: None,
            has_ever_streamed: false,
            folder: folder.to_string(),
            last_escape_time: None,
            paste_burst: crate::paste_burst::PasteBurst::new(),
            paste_accumulator: String::new(),
            last_paste_time: None,
            paste_burst_consecutive: 0,
            paste_burst_active: false,
            last_render_state: AppState::Input,
            question_dialog: None,
            question_response_tx: None,
            pending_user_messages: std::collections::VecDeque::new(),
            pending_follow_up_messages: std::collections::VecDeque::new(),
            update_prompt: None,
            todo_items: Vec::new(),
            todo_scroll_offset: 0,
            show_hardware_cursor: true,
            usage_display: None,
            pending_history_lines: Vec::new(),
            share_in_progress: false,
            last_detected_modifiers: KeyModifiers::NONE,
            last_key_event_time: None,
            just_processed_newline: false,
            last_newline_time: None,
        })
    }

    /// Internal constructor that skips raw mode / alternate screen.
    #[allow(dead_code)]
    pub fn new_inner(
        provider: &str,
        model_id: &str,
        app_name: &str,
        version: &str,
        context_file_names: Vec<String>,
        skill_names: Vec<String>,
        cwd: &str,
        home_dir: Option<String>,
        thinking_level: &str,
        autocomplete_provider: Option<Box<dyn AutocompleteProvider>>,
        folder: &str,
        agent_mode: &str,
    ) -> Self {
        let mut editor = Editor::new();
        editor.show_prompt = true;
        editor.prompt = "\u{276f} ".to_string();
        if let Some(provider) = autocomplete_provider {
            editor.set_autocomplete_provider(provider);
        }
        let chat = crate::components::chat::ChatView::new();
        Self {
            chat_lines_written: 0,
            confirm_quit: false,
            chat,
            editor,
            state: AppState::Input,
            provider: provider.to_string(),
            model_id: model_id.to_string(),
            app_name: app_name.to_string(),
            version: version.to_string(),
            startup_header_added: false,
            context_file_names,
            skill_names,
            cwd: cwd.to_string(),
            home_dir,
            total_input: 0,
            total_output: 0,
            total_cache_read: 0,
            total_cache_write: 0,
            context_percent: Some(0.0),
            context_window: 1_000_000,
            git_branch: None,
            session_name: None,
            thinking_level: thinking_level.to_string(),
            auto_compact: true,
            selection: None,
            tree_view: None,
            api_key_provider: None,
            api_key_input: String::new(),
            agent_mode: agent_mode.to_string(),
            cached_lines_entry_count: 0,
            cached_lines_committed: 0,
            last_render_width: 0,
            autocomplete_space_lines: 0,
            status_text: None,
            status_frame: 0,
            agent_start_time: None,
            has_ever_streamed: false,
            folder: folder.to_string(),
            last_escape_time: None,
            paste_burst: crate::paste_burst::PasteBurst::new(),
            paste_accumulator: String::new(),
            last_paste_time: None,
            paste_burst_consecutive: 0,
            paste_burst_active: false,
            last_render_state: AppState::Input,
            question_dialog: None,
            question_response_tx: None,
            pending_user_messages: std::collections::VecDeque::new(),
            pending_follow_up_messages: std::collections::VecDeque::new(),
            update_prompt: None,
            todo_items: Vec::new(),
            todo_scroll_offset: 0,
            show_hardware_cursor: true,
            usage_display: None,
            pending_history_lines: Vec::new(),
            share_in_progress: false,
            last_detected_modifiers: KeyModifiers::NONE,
            last_key_event_time: None,
            just_processed_newline: false,
            last_newline_time: None,
        }
    }

    /// Clean up terminal state and position shell prompt below all TUI content.
    pub fn cleanup(&mut self) {
        crate::keyboard_enhancement::disable();
        let _ = disable_raw_mode();
        let _ = writeln!(std::io::stdout());
        let _ = std::io::stdout().flush();
    }

    /// Set todo items with auto-scroll
    pub fn set_todo_items(&mut self, todos: Vec<serde_json::Value>) {
        self.todo_items = todos;
        self.todo_items.sort_by_key(|t| {
            let s = t.get("status").and_then(|v| v.as_str()).unwrap_or("");
            if s == "in_progress" { 0 } else { 1 }
        });
        let active_count = self
            .todo_items
            .iter()
            .filter(|t| {
                let s = t.get("status").and_then(|v| v.as_str()).unwrap_or("");
                s != "completed" && s != "cancelled"
            })
            .count();

        if active_count == 0 {
            return;
        }
        let max_scroll = active_count.saturating_sub(5);

        if let Some(idx) = self
            .todo_items
            .iter()
            .position(|t| t.get("status").and_then(|v| v.as_str()).unwrap_or("") == "in_progress")
        {
            let active_before = self
                .todo_items
                .iter()
                .take(idx)
                .filter(|t| {
                    let s = t.get("status").and_then(|v| v.as_str()).unwrap_or("");
                    s != "completed" && s != "cancelled"
                })
                .count();
            self.todo_scroll_offset = (active_before + 1).saturating_sub(5);
            self.todo_scroll_offset = self.todo_scroll_offset.min(max_scroll);
        } else {
            self.todo_scroll_offset = self.todo_scroll_offset.min(max_scroll);
        }
    }

    /// Replace the autocomplete provider (e.g., after skill commands are toggled).
    pub fn set_autocomplete_provider(&mut self, provider: Box<dyn AutocompleteProvider>) {
        self.editor.set_autocomplete_provider(provider);
        // Re-trigger autocomplete so the new commands appear immediately if / was typed
        self.editor.trigger_autocomplete();
    }

    /// Reset scrollback tracking state
    pub fn reset_scrollback_state(&mut self) {
        self.cached_lines_entry_count = 0;
        self.cached_lines_committed = 0;
        self.last_render_width = 0;
        self.has_ever_streamed = false;
    }

    /// Start an interactive selection dialog
    pub fn start_selection(&mut self, select_list: SelectList) {
        self.selection = Some(select_list);
        self.state = AppState::Selecting;
    }

    /// Get the current selection list, if any
    pub fn selection_list(&self) -> Option<&SelectList> {
        self.selection.as_ref()
    }

    /// Cancel the current selection dialog
    pub fn cancel_selection(&mut self) {
        self.selection = None;
        self.state = AppState::Input;
    }

    /// Start a tree view session (e.g., /tree command)
    pub fn start_tree_view(&mut self, tree_view: TreeView) {
        self.tree_view = Some(tree_view);
        self.state = AppState::TreeSelecting;
    }

    /// Start an update prompt dialog
    pub fn start_update_prompt(&mut self, prompt: crate::components::UpdatePromptState) {
        self.update_prompt = Some(prompt);
        self.state = AppState::UpdatePrompt;
    }

    /// Cancel the current update prompt
    pub fn cancel_update_prompt(&mut self) {
        self.update_prompt = None;
        self.state = AppState::Input;
    }

    /// Cancel the current tree view
    pub fn cancel_tree_view(&mut self) {
        self.tree_view = None;
        self.state = AppState::Input;
    }

    /// Handle key events during selection mode
    fn handle_key_selection(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> Option<TuiAction> {
        if modifiers.contains(KeyModifiers::CONTROL) {
            match code {
                KeyCode::Char('c') | KeyCode::Char('d') => {
                    self.cancel_selection();
                    return None;
                }
                _ => {}
            }
        }

        match code {
            KeyCode::Up => {
                if let Some(ref mut sel) = self.selection {
                    sel.previous();
                }
                None
            }
            KeyCode::Down => {
                if let Some(ref mut sel) = self.selection {
                    sel.next();
                }
                None
            }
            KeyCode::PageUp => {
                if let Some(ref mut sel) = self.selection {
                    sel.prev_page();
                }
                None
            }
            KeyCode::PageDown => {
                if let Some(ref mut sel) = self.selection {
                    sel.next_page();
                }
                None
            }
            KeyCode::Home => {
                if let Some(ref mut sel) = self.selection {
                    sel.first();
                }
                None
            }
            KeyCode::End => {
                if let Some(ref mut sel) = self.selection {
                    sel.last();
                }
                None
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(ref sel) = self.selection
                    && let Some(item) = sel.selected()
                {
                    let idx = sel.selected_index;
                    let value = item.value.clone();
                    self.cancel_selection();
                    return Some(TuiAction::SelectionResult(idx, value));
                }
                self.cancel_selection();
                None
            }
            KeyCode::Backspace => {
                if let Some(ref mut sel) = self.selection {
                    sel.pop_search_char();
                }
                None
            }
            KeyCode::Char(c) if !c.is_ascii_control() => {
                if let Some(ref mut sel) = self.selection {
                    sel.push_search_char(c);
                }
                None
            }
            KeyCode::Esc => {
                if let Some(ref mut sel) = self.selection {
                    if sel.has_search() {
                        sel.clear_search();
                        None
                    } else {
                        self.cancel_selection();
                        Some(TuiAction::SelectionCancelled)
                    }
                } else {
                    self.cancel_selection();
                    None
                }
            }
            _ => None,
        }
    }

    /// Handle key events during tree selection mode (/tree command)
    fn handle_key_tree_selecting(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> Option<TuiAction> {
        if self
            .tree_view
            .as_ref()
            .is_some_and(|tv| tv.edit_label_entry_id.is_some())
        {
            match code {
                KeyCode::Enter => {
                    let entry_id = self
                        .tree_view
                        .as_ref()
                        .and_then(|tv| tv.edit_label_entry_id.clone());
                    let label = self
                        .tree_view
                        .as_ref()
                        .map(|tv| tv.edit_label_buffer.clone());
                    self.cancel_tree_view();
                    if let Some(eid) = entry_id {
                        let lbl = label.filter(|s| !s.is_empty());
                        return Some(TuiAction::SelectionResult(
                            0,
                            format!("__label__{}:{}", eid, lbl.unwrap_or_default()),
                        ));
                    }
                    return None;
                }
                KeyCode::Esc => {
                    if let Some(ref mut tv) = self.tree_view {
                        tv.cancel_edit_label();
                    }
                    return None;
                }
                KeyCode::Backspace => {
                    if let Some(ref mut tv) = self.tree_view {
                        tv.edit_label_buffer.pop();
                    }
                    return None;
                }
                KeyCode::Char(c) => {
                    if let Some(ref mut tv) = self.tree_view {
                        tv.edit_label_buffer.push(c);
                    }
                    return None;
                }
                _ => return None,
            }
        }

        if modifiers.contains(KeyModifiers::CONTROL) {
            match code {
                KeyCode::Char('c') => {
                    self.cancel_tree_view();
                    return None;
                }
                KeyCode::Char('d') => {
                    if let Some(ref mut tv) = self.tree_view {
                        tv.set_filter_mode(super::types::TreeFilterMode::Default);
                    }
                    return None;
                }
                KeyCode::Char('t') => {
                    if let Some(ref mut tv) = self.tree_view {
                        let cur = tv.filter_mode;
                        tv.set_filter_mode(if cur == super::types::TreeFilterMode::NoTools {
                            super::types::TreeFilterMode::Default
                        } else {
                            super::types::TreeFilterMode::NoTools
                        });
                    }
                    return None;
                }
                KeyCode::Char('u') => {
                    if let Some(ref mut tv) = self.tree_view {
                        let cur = tv.filter_mode;
                        tv.set_filter_mode(if cur == super::types::TreeFilterMode::UserOnly {
                            super::types::TreeFilterMode::Default
                        } else {
                            super::types::TreeFilterMode::UserOnly
                        });
                    }
                    return None;
                }
                KeyCode::Char('l') => {
                    if let Some(ref mut tv) = self.tree_view {
                        let cur = tv.filter_mode;
                        tv.set_filter_mode(if cur == super::types::TreeFilterMode::LabeledOnly {
                            super::types::TreeFilterMode::Default
                        } else {
                            super::types::TreeFilterMode::LabeledOnly
                        });
                    }
                    return None;
                }
                KeyCode::Char('a') => {
                    if let Some(ref mut tv) = self.tree_view {
                        let cur = tv.filter_mode;
                        tv.set_filter_mode(if cur == super::types::TreeFilterMode::All {
                            super::types::TreeFilterMode::Default
                        } else {
                            super::types::TreeFilterMode::All
                        });
                    }
                    return None;
                }
                KeyCode::Char('o') => {
                    if let Some(ref mut tv) = self.tree_view {
                        tv.cycle_filter();
                    }
                    return None;
                }
                KeyCode::Left => {
                    if let Some(ref mut tv) = self.tree_view {
                        tv.fold_selected();
                    }
                    return None;
                }
                KeyCode::Right => {
                    if let Some(ref mut tv) = self.tree_view {
                        tv.unfold_selected();
                    }
                    return None;
                }
                _ => {}
            }
        }

        match code {
            KeyCode::Up => {
                if let Some(ref mut tv) = self.tree_view {
                    tv.move_up();
                }
                None
            }
            KeyCode::Down => {
                if let Some(ref mut tv) = self.tree_view {
                    tv.move_down();
                }
                None
            }
            KeyCode::PageUp => {
                if let Some(ref mut tv) = self.tree_view {
                    tv.page_up();
                }
                None
            }
            KeyCode::PageDown => {
                if let Some(ref mut tv) = self.tree_view {
                    tv.page_down();
                }
                None
            }
            KeyCode::Home => {
                if let Some(ref mut tv) = self.tree_view {
                    tv.go_to_home();
                }
                None
            }
            KeyCode::End => {
                if let Some(ref mut tv) = self.tree_view {
                    tv.go_to_end();
                }
                None
            }
            KeyCode::Enter => {
                if let Some(ref tv) = self.tree_view
                    && let Some(id) = tv.selected_entry_id()
                {
                    let val = id.to_string();
                    self.cancel_tree_view();
                    return Some(TuiAction::SelectionResult(0, val));
                }
                self.cancel_tree_view();
                None
            }
            KeyCode::Char('L') if !modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(ref mut tv) = self.tree_view {
                    tv.start_edit_label();
                }
                None
            }
            KeyCode::Char('T') if !modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(ref mut tv) = self.tree_view {
                    tv.toggle_label_timestamps();
                }
                None
            }
            KeyCode::Esc => {
                if let Some(ref mut tv) = self.tree_view {
                    if tv.search_query.is_empty() {
                        self.cancel_tree_view();
                    } else {
                        tv.clear_search();
                    }
                } else {
                    self.cancel_tree_view();
                }
                None
            }
            KeyCode::Backspace => {
                if let Some(ref mut tv) = self.tree_view {
                    tv.pop_search();
                }
                None
            }
            KeyCode::Char(c)
                if !modifiers.contains(KeyModifiers::CONTROL)
                    && !modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(ref mut tv) = self.tree_view {
                    tv.append_search(c);
                }
                None
            }
            _ => None,
        }
    }

    /// Update the question dialog selection
    fn question_navigate(&mut self, delta: isize) {
        if let Some(ref mut dialog) = self.question_dialog
            && let Some(q) = dialog.current()
            && !q.options.is_empty()
        {
            let current = q.selected.first().copied().unwrap_or(0);
            let count = q.options.len();
            let next = ((current as isize + delta).rem_euclid(count as isize)) as usize;
            if let Some(ref mut d) = self.question_dialog
                && let Some(q) = d.questions.get_mut(d.current_index)
            {
                if q.multiple {
                    q.selected.clear();
                }
                q.selected = vec![next];
            }
        }
    }

    /// Toggle multiple selection for current option
    fn question_toggle(&mut self) {
        if let Some(ref mut dialog) = self.question_dialog
            && let Some(q) = dialog.questions.get_mut(dialog.current_index)
            && q.multiple
            && !q.options.is_empty()
        {
            let current = q.selected.first().copied().unwrap_or(0);
            if let Some(pos) = q.selected.iter().position(|&i| i == current) {
                q.selected.remove(pos);
            } else {
                q.selected.push(current);
                q.selected.sort();
            }
        }
    }

    /// Confirm current question and advance to next, or complete
    fn question_confirm(&mut self) -> Option<Vec<Vec<String>>> {
        let dialog = self.question_dialog.as_mut()?;
        let q = dialog.questions.get(dialog.current_index)?;

        let answers: Vec<String> = if dialog.custom_mode {
            vec![dialog.custom_input.clone().unwrap_or_default()]
        } else if q.multiple {
            q.selected
                .iter()
                .map(|&i| q.options[i].label.clone())
                .collect()
        } else {
            let idx = q.selected.first().copied().unwrap_or(0);
            vec![q.options[idx].label.clone()]
        };

        if dialog.is_last() {
            let all_answers = vec![answers];
            Some(all_answers)
        } else {
            dialog.current_index += 1;
            dialog.custom_mode = false;
            dialog.custom_input = None;
            None
        }
    }

    /// Handle key events when in Questioning state
    fn handle_question_key(
        &mut self,
        code: KeyCode,
        _modifiers: KeyModifiers,
    ) -> Option<TuiAction> {
        if self.question_dialog.is_none() {
            self.state = AppState::Input;
            return None;
        }

        let in_custom = self.question_dialog.as_ref().is_some_and(|d| d.custom_mode);

        match code {
            KeyCode::Esc => {
                if let Some(tx) = self.question_response_tx.take() {
                    let _ = tx.send(Err("User cancelled".to_string()));
                }
                self.question_dialog = None;
                self.state = AppState::Input;
            }
            KeyCode::Up | KeyCode::Char('k') if !in_custom => {
                self.question_navigate(-1);
            }
            KeyCode::Down | KeyCode::Char('j') if !in_custom => {
                self.question_navigate(1);
            }
            KeyCode::Char(' ') if !in_custom => {
                if self
                    .question_dialog
                    .as_ref()
                    .is_some_and(|d| d.current().is_some_and(|q| q.multiple))
                {
                    self.question_toggle();
                } else {
                    return self.handle_question_confirm();
                }
            }
            KeyCode::Enter => {
                if in_custom {
                    return self.handle_question_confirm();
                }
                return self.handle_question_confirm();
            }
            KeyCode::Char('/') => {
                if let Some(ref mut dialog) = self.question_dialog {
                    dialog.custom_mode = true;
                    dialog.custom_input = Some(String::new());
                }
            }
            KeyCode::Char(c) if in_custom => {
                if let Some(ref mut dialog) = self.question_dialog {
                    let input = dialog.custom_input.get_or_insert_with(String::new);
                    input.push(c);
                }
            }
            KeyCode::Backspace if in_custom => {
                if let Some(ref mut dialog) = self.question_dialog
                    && let Some(ref mut input) = dialog.custom_input
                {
                    input.pop();
                }
            }
            _ => {}
        }
        None
    }

    /// Handle question confirmation: collect answers and advance/complete
    fn handle_question_confirm(&mut self) -> Option<TuiAction> {
        let answers = self.question_confirm();
        if let Some(all_answers) = answers {
            if let Some(tx) = self.question_response_tx.take() {
                let _ = tx.send(Ok(all_answers));
            }
            self.question_dialog = None;
            self.state = AppState::Input;
            self.autocomplete_space_lines = 0;
            self.chat.discard_active_stream();
        }
        None
    }

    /// Handle key events during update prompt mode
    fn handle_key_update_prompt(
        &mut self,
        code: KeyCode,
        _modifiers: KeyModifiers,
    ) -> Option<TuiAction> {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(ref mut p) = self.update_prompt {
                    p.previous();
                }
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(ref mut p) = self.update_prompt {
                    p.next();
                }
                None
            }
            KeyCode::Char('1') => {
                self.update_prompt = None;
                self.state = AppState::Input;
                Some(TuiAction::UpdateResponse(UpdateChoice::UpdateNow))
            }
            KeyCode::Char('2') => {
                // Skip — just hide the prompt, remind again on next startup
                self.cancel_update_prompt();
                None
            }
            KeyCode::Char('3') => {
                // Don't remind for this version (persist to cache)
                self.update_prompt = None;
                self.state = AppState::Input;
                Some(TuiAction::UpdateResponse(UpdateChoice::Dismiss))
            }
            KeyCode::Enter => {
                match self.update_prompt.as_ref().map(|p| p.selected) {
                    Some(0) => {
                        // Update now
                        self.update_prompt = None;
                        self.state = AppState::Input;
                        Some(TuiAction::UpdateResponse(UpdateChoice::UpdateNow))
                    }
                    Some(1) => {
                        // Skip — just hide the prompt, remind again on next startup
                        self.cancel_update_prompt();
                        None
                    }
                    _ => {
                        // Don't remind for this version (persist to cache)
                        self.update_prompt = None;
                        self.state = AppState::Input;
                        Some(TuiAction::UpdateResponse(UpdateChoice::Dismiss))
                    }
                }
            }
            KeyCode::Esc => {
                self.cancel_update_prompt();
                None
            }
            _ => None,
        }
    }

    /// Handle a key event, returning an action if the input results in submission or quit
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Option<TuiAction> {
        if self.state == AppState::Questioning {
            return self.handle_question_key(code, modifiers);
        }

        if self.state == AppState::UpdatePrompt {
            return self.handle_key_update_prompt(code, modifiers);
        }

        if (modifiers == KeyModifiers::CONTROL && code == KeyCode::Char('c'))
            || code == KeyCode::Char('\x03')
        {
            if self.state == AppState::Streaming {
                return Some(TuiAction::Quit);
            }
            if !self.editor.text().trim().is_empty() {
                self.editor.clear();
                return None;
            }
            if !self.confirm_quit {
                self.confirm_quit = true;
                return None;
            }
            self.confirm_quit = false;
            return Some(TuiAction::Quit);
        }

        if (modifiers == KeyModifiers::CONTROL && code == KeyCode::Char('d'))
            || code == KeyCode::Char('\x04')
        {
            if self.state == AppState::TreeSelecting {
                // handled by handle_key_tree_selecting
            } else if self.state == AppState::Streaming || self.editor.text().trim().is_empty() {
                return Some(TuiAction::Quit);
            } else {
                return None;
            }
        }

        if modifiers == KeyModifiers::CONTROL && code == KeyCode::Char('o') {
            if self.state == AppState::TreeSelecting {
                // handled by handle_key_tree_selecting
            } else {
                self.chat.toggle_tool_expansion();
            }
            return None;
        }

        // Any other key press cancels pending quit confirmation
        self.confirm_quit = false;

        if code == KeyCode::Esc {
            if self.state == AppState::Streaming {
                return Some(TuiAction::Interrupt);
            }
            if self.state == AppState::Input && self.editor.text().trim().is_empty() {
                let now = std::time::Instant::now();
                let is_double = self
                    .last_escape_time
                    .is_some_and(|t| now.duration_since(t).as_millis() < 500);
                self.last_escape_time = Some(now);
                if is_double {
                    self.last_escape_time = None;
                    return Some(TuiAction::OpenTree);
                }
                return None;
            }
        }

        if self.state == AppState::TreeSelecting {
            return self.handle_key_tree_selecting(code, modifiers);
        }

        if self.state == AppState::Selecting {
            return self.handle_key_selection(code, modifiers);
        }

        if self.state == AppState::ApiKeyInput {
            match code {
                KeyCode::Esc => {
                    self.state = AppState::Input;
                    self.api_key_input.clear();
                }
                KeyCode::Enter => {
                    let key = std::mem::take(&mut self.api_key_input);
                    self.state = AppState::Input;
                    return Some(TuiAction::ApiKeySubmit(key));
                }
                KeyCode::Char(c) => {
                    self.api_key_input.push(c);
                }
                KeyCode::Backspace => {
                    self.api_key_input.pop();
                }
                _ => {}
            }
            if modifiers == KeyModifiers::CONTROL
                && matches!(code, KeyCode::Char('c') | KeyCode::Char('d'))
            {
                self.state = AppState::Input;
                self.api_key_input.clear();
            }
            return None;
        }

        match code {
            // Shift+Enter: insert a new line at the cursor.
            // Ctrl+Enter: no-op (not match, no action).
            KeyCode::Enter if modifiers.contains(KeyModifiers::SHIFT) => {
                if self.editor.is_autocomplete_active() {
                    self.editor.cancel_autocomplete();
                }
                self.force_flush_paste_accumulator();
                self.editor.insert_newline_auto_indent();
            }
            // '\n' and '\r' are unprintable control characters that
            // should never be inserted into the visible text buffer.
            // On Windows without keyboard enhancement, both Ctrl+Enter
            // and Shift+Enter may be reported as \n+NONE (or \n+CONTROL).
            // We rely on last_detected_modifiers from track_modifiers
            // to distinguish them via GetAsyncKeyState.  When modifiers
            // are ambiguous, default to newline (this prevents data loss
            // and matches the expected behavior for the common case
            // of Shift+Enter on non-enhanced Windows terminals).
            KeyCode::Char('\n') | KeyCode::Char('\r') => {
                if self.editor.is_autocomplete_active() {
                    self.editor.cancel_autocomplete();
                }
                self.force_flush_paste_accumulator();
                if modifiers.contains(KeyModifiers::SHIFT)
                    || self.last_detected_modifiers.contains(KeyModifiers::SHIFT)
                {
                    self.editor.insert_newline_auto_indent();
                } else if modifiers.contains(KeyModifiers::CONTROL)
                    || self.last_detected_modifiers.contains(KeyModifiers::CONTROL)
                {
                    // Ctrl+Enter: no-op.
                } else {
                    // Ambiguous modifier — likely \n from a modified
                    // Enter on a terminal that doesn't report modifiers
                    // correctly.  Default to newline to avoid data loss.
                    self.editor.insert_newline_auto_indent();
                }
            }
            KeyCode::Enter => {
                // Flush any paste-accumulated text into the editor so
                // that submit_input() sees the full buffer content.
                self.force_flush_paste_accumulator();

                // Ctrl+Enter: no-op (neither submit nor insert newline).
                // Check AFTER flushing so pending text isn't lost.
                if modifiers.contains(KeyModifiers::CONTROL)
                    || self.last_detected_modifiers.contains(KeyModifiers::CONTROL)
                {
                    return None;
                }

                if self.state == AppState::Streaming {
                    let text = self.editor.text().to_string();
                    self.editor.clear();
                    if !text.trim().is_empty() {
                        if modifiers == KeyModifiers::ALT {
                            return Some(TuiAction::QueueFollowUp(text));
                        }
                        return Some(TuiAction::QueueMessage(text));
                    }
                    return None;
                }
                if self.editor.is_autocomplete_active() {
                    self.editor.autocomplete_apply_completion();
                    let trimmed = self.editor.text().trim().to_string();
                    if trimmed.starts_with('/')
                        && !trimmed.starts_with("/skill:")
                        && trimmed != "/goal"
                    {
                        return self.submit_input();
                    }
                    return None;
                }
                return self.submit_input();
            }
            KeyCode::Tab => {
                if self.editor.is_autocomplete_active() {
                    self.editor.autocomplete_next();
                } else {
                    let text = self.editor.text().to_string();
                    let trimmed = text.trim_start();
                    if trimmed.starts_with('/') {
                        self.editor.trigger_autocomplete();
                        if self.editor.is_autocomplete_active() {
                            return None;
                        }
                    }
                    if trimmed.is_empty() {
                        return Some(TuiAction::CycleMode);
                    }
                    self.editor.insert_tab();
                }
            }
            KeyCode::Backspace => {
                self.editor.delete_before();
                if self.editor.is_autocomplete_active()
                    || self.editor.text().trim_start().starts_with('/')
                {
                    self.editor.trigger_autocomplete();
                }
            }
            KeyCode::Delete => self.editor.delete_after(),

            KeyCode::Left if modifiers.contains(KeyModifiers::CONTROL) => {
                self.editor.cursor_word_left();
            }
            KeyCode::Right if modifiers.contains(KeyModifiers::CONTROL) => {
                self.editor.cursor_word_right();
            }

            KeyCode::Left => self.editor.cursor_left(),
            KeyCode::Right => self.editor.cursor_right(),
            KeyCode::Up => {
                if self.editor.is_autocomplete_active() {
                    self.editor.autocomplete_previous();
                } else if self.editor.history_index.is_some() {
                    self.editor.history_previous();
                } else if self.editor.buffer.is_empty() {
                    self.editor.history_previous();
                } else {
                    self.editor.cursor_up();
                }
            }
            KeyCode::Down => {
                if self.editor.is_autocomplete_active() {
                    self.editor.autocomplete_next();
                } else if self.editor.history_index.is_some() {
                    self.editor.history_next();
                } else if self.editor.buffer.is_empty() {
                    self.editor.history_next();
                } else {
                    self.editor.cursor_down();
                }
            }
            KeyCode::Home => self.editor.cursor_line_start(),
            KeyCode::End => self.editor.cursor_line_end(),

            KeyCode::PageUp => {
                if self.editor.line_count() > 1 {
                    self.editor.page_up();
                } else {
                    self.chat.scroll_up(10);
                }
            }
            KeyCode::PageDown => {
                if self.editor.line_count() > 1 {
                    self.editor.page_down();
                } else {
                    self.chat.scroll_down(10);
                }
            }

            KeyCode::Char(c) => {
                if modifiers.contains(KeyModifiers::ALT) {
                    return None;
                }
                if !modifiers.contains(KeyModifiers::CONTROL) {
                    // Reject ASCII control characters (0x00-0x1F) except
                    // tab (\t, 0x09), newline (\n, 0x0A), and carriage
                    // return (\r, 0x0D) which can arrive as keyboard input
                    // on some terminals.  Other control characters (NUL,
                    // BEL, BS, etc.) have no visible glyph and would
                    // corrupt the buffer if inserted.
                    if (c as u32) <= 0x1F && !matches!(c, '\t' | '\n' | '\r') {
                        return None;
                    }
                    self.editor.insert_char(c);
                }
            }

            KeyCode::Esc => {
                if self.editor.is_autocomplete_active() {
                    self.editor.cancel_autocomplete();
                } else if self.editor.mark.is_some() {
                    self.editor.mark = None;
                }
            }

            _ => {}
        }

        // Ctrl+L: select model
        if modifiers == KeyModifiers::CONTROL && code == KeyCode::Char('l') {
            return Some(TuiAction::SelectModel);
        }

        // Shift+Tab: cycle thinking level
        if code == KeyCode::Tab && modifiers.contains(KeyModifiers::SHIFT) {
            return Some(TuiAction::CycleThinking);
        }

        // Ctrl+Shift+P: cycle model backward
        if modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) && code == KeyCode::Char('p')
        {
            return Some(TuiAction::CycleModelBackward);
        }

        // Ctrl+key shortcuts (emacs-style editing)
        if modifiers.contains(KeyModifiers::CONTROL)
            && !matches!(
                code,
                KeyCode::Char('c') | KeyCode::Char('d') | KeyCode::Char('l')
            )
        {
            match code {
                KeyCode::Char('n') => self.editor.cursor_down(),
                KeyCode::Char('b') => self.editor.cursor_left(),
                KeyCode::Char('f') => self.editor.cursor_right(),
                KeyCode::Char('p') => self.editor.cursor_up(),
                KeyCode::Char('a') => self.editor.cursor_line_start(),
                KeyCode::Char('e') => self.editor.cursor_line_end(),
                KeyCode::Char('d') => self.editor.delete_after(),
                KeyCode::Char('h') => self.editor.delete_before(),
                KeyCode::Char('k') => self.editor.delete_to_line_end(),
                KeyCode::Char('u') => self.editor.delete_to_line_start(),
                KeyCode::Char('w') => self.editor.delete_word_before(),
                KeyCode::Char('z') => self.editor.undo(),
                KeyCode::Char('s') => {
                    self.editor.mark = Some(0);
                    self.editor.cursor_buffer_end();
                }
                _ => {}
            }
        }

        None
    }

    /// Handle a paste event: insert pasted text into the editor buffer.
    pub fn handle_paste(&mut self, text: &str) {
        if matches!(
            self.state,
            AppState::Selecting | AppState::TreeSelecting | AppState::UpdatePrompt
        ) {
            return;
        }

        if self.state == AppState::ApiKeyInput {
            self.api_key_input.push_str(text);
            return;
        }
        // Clear any residual paste accumulator so it doesn't get flushed
        // after the paste, inserting stale content alongside the real paste.
        self.paste_accumulator.clear();
        let pasted = text.replace("\r\n", "\n").replace('\r', "\n");
        self.editor.insert_str(&pasted);
        self.last_paste_time = Some(Instant::now());
        if self.editor.is_autocomplete_active() || self.editor.text().trim_start().starts_with('/')
        {
            self.editor.trigger_autocomplete();
        }
    }

    /// Handle a regular printable character. Accumulate for paste burst detection.
    /// Tracks consecutive fast chars (<50ms apart). When 3+ consecutive fast
    /// chars are seen, sets `paste_burst_active` so the next flush forces
    /// PasteElement creation (even for below-threshold content), ensuring the
    /// first batch of a multi-batch paste creates a placeholder.
    pub fn handle_char_for_paste(&mut self, c: char, now: Instant) {
        if let Some(last) = self.last_paste_time
            && now.duration_since(last).as_millis() < 50
        {
            self.paste_burst_consecutive += 1;
            if self.paste_burst_consecutive >= 3 {
                self.paste_burst_active = true;
            }
        } else {
            self.paste_burst_consecutive = 1;
            self.paste_burst_active = false;
        }
        self.paste_accumulator.push(c);
        self.last_paste_time = Some(now);
    }

    /// Force-flush paste accumulator, ignoring the 50ms timing threshold.
    /// Ensures accumulated typed text is inserted into the editor buffer before
    /// a CONTROL/ALT key or Enter is processed, preventing the key handler from
    /// operating on an empty buffer with pending text.
    pub fn force_flush_paste_accumulator(&mut self) {
        if self.paste_accumulator.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.paste_accumulator);
        if self.paste_burst_active {
            self.editor.insert_str_paste_burst(&text);
        } else {
            self.editor.insert_str(&text);
        }
        self.paste_burst_active = false;
        self.paste_burst_consecutive = 0;
        self.last_paste_time = Some(Instant::now());
        let trimmed = self.editor.text().trim_start();
        if trimmed.starts_with('/') && !trimmed[1..].contains(' ') {
            self.editor.trigger_autocomplete();
        } else {
            self.editor.cancel_autocomplete();
        }
    }

    /// Finalize paste accumulator
    pub fn finalize_paste_accumulator(&mut self, now: Instant) -> bool {
        if self.paste_accumulator.is_empty() {
            return true;
        }
        if let Some(t) = self.last_paste_time
            && now.duration_since(t).as_millis() > 50
        {
            let text = std::mem::take(&mut self.paste_accumulator);
            if self.paste_burst_active {
                self.editor.insert_str_paste_burst(&text);
            } else {
                self.editor.insert_str(&text);
            }
            self.paste_burst_active = false;
            self.paste_burst_consecutive = 0;
            self.last_paste_time = Some(now);
            let trimmed = self.editor.text().trim_start();
            if trimmed.starts_with('/') && !trimmed[1..].contains(' ') {
                self.editor.trigger_autocomplete();
            } else {
                self.editor.cancel_autocomplete();
            }
            return true;
        }
        false
    }

    /// Submit the current editor content and return it
    pub(crate) fn submit_input(&mut self) -> Option<TuiAction> {
        let text = self.editor.full_text();
        if text.trim().is_empty() {
            return None;
        }
        let is_slash = text.trim().starts_with('/');
        self.editor.push_history(text.clone());
        self.editor.clear();
        self.autocomplete_space_lines = 0;
        if !is_slash {
            self.state = AppState::Streaming;
            self.update_terminal_title();
        }
        self.has_ever_streamed = true;
        Some(TuiAction::Submit(text))
    }
}
