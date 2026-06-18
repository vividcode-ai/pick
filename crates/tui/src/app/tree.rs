//! Tree view state methods and tree rendering

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::app::types::{TreeFilterMode, TreeView, TreeViewItem};

use super::types::TuiApp;

impl TreeView {
    pub fn new(
        items: Vec<TreeViewItem>,
        current_leaf_id: Option<String>,
        active_path_ids: Vec<String>,
    ) -> Self {
        let count = items.len();
        let visible: Vec<usize> = (0..count).collect();
        Self {
            items,
            visible_indices: visible,
            selected_index: 0,
            current_leaf_id,
            active_path_ids,
            folded_ids: std::collections::HashSet::new(),
            filter_mode: TreeFilterMode::Default,
            search_query: String::new(),
            show_label_timestamps: false,
            edit_label_entry_id: None,
            edit_label_buffer: String::new(),
        }
    }

    pub fn selected_entry_id(&self) -> Option<&str> {
        self.visible_indices
            .get(self.selected_index)
            .map(|&idx| self.items[idx].entry_id.as_str())
    }
    pub fn visible_count(&self) -> usize {
        self.visible_indices.len()
    }

    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }
    pub fn move_down(&mut self) {
        if self.selected_index + 1 < self.visible_indices.len() {
            self.selected_index += 1;
        }
    }
    pub fn page_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(10);
    }
    pub fn page_down(&mut self) {
        let max = self.visible_indices.len().saturating_sub(1);
        self.selected_index = (self.selected_index + 10).min(max);
    }

    pub fn go_to_home(&mut self) {
        self.selected_index = 0;
    }
    pub fn go_to_end(&mut self) {
        self.selected_index = self.visible_count().saturating_sub(1);
    }

    pub fn toggle_fold(&mut self) {
        if let Some(idx) = self.visible_indices.get(self.selected_index) {
            let item = &self.items[*idx];
            if item.has_children {
                let id = &item.entry_id;
                if self.folded_ids.contains(id) {
                    self.folded_ids.remove(id);
                } else {
                    self.folded_ids.insert(id.clone());
                }
                self.rebuild_visible();
            }
        }
    }

    pub fn fold_selected(&mut self) {
        if let Some(idx) = self.visible_indices.get(self.selected_index) {
            let item = &self.items[*idx];
            if item.has_children && !self.folded_ids.contains(&item.entry_id) {
                self.folded_ids.insert(item.entry_id.clone());
                self.rebuild_visible();
            }
        }
    }

    pub fn unfold_selected(&mut self) {
        if let Some(idx) = self.visible_indices.get(self.selected_index) {
            let item = &self.items[*idx];
            if self.folded_ids.contains(&item.entry_id) {
                self.folded_ids.remove(&item.entry_id);
                self.rebuild_visible();
            }
        }
    }

    fn is_settings_kind(k: &str) -> bool {
        matches!(
            k,
            "model_change"
                | "thinking_level_change"
                | "custom"
                | "session_info"
                | "leaf_change"
                | "label"
        )
    }

    fn rebuild_visible(&mut self) {
        let search_tokens: Vec<String> = if self.search_query.is_empty() {
            Vec::new()
        } else {
            self.search_query
                .to_lowercase()
                .split_whitespace()
                .map(|s| s.to_string())
                .collect()
        };
        let folded = &self.folded_ids;
        self.visible_indices = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                match self.filter_mode {
                    TreeFilterMode::UserOnly if item.kind_str != "user" => return false,
                    TreeFilterMode::NoTools
                        if Self::is_settings_kind(&item.kind_str)
                            || item.kind_str == "tool_result" =>
                    {
                        return false;
                    }
                    TreeFilterMode::Default if Self::is_settings_kind(&item.kind_str) => {
                        return false;
                    }
                    TreeFilterMode::LabeledOnly if item.label.is_none() => return false,
                    _ => {}
                }
                if !folded.is_empty() {
                    let mut current = item.parent_id.as_deref();
                    while let Some(pid) = current {
                        if folded.contains(pid) {
                            return false;
                        }
                        current = self
                            .items
                            .iter()
                            .find(|i| i.entry_id == pid)
                            .and_then(|i| i.parent_id.as_deref());
                    }
                }
                if !search_tokens.is_empty() {
                    let lower = item.searchable_text.to_lowercase();
                    if !search_tokens.iter().all(|t| lower.contains(t.as_str())) {
                        return false;
                    }
                }
                true
            })
            .map(|(idx, _)| idx)
            .collect();
        if !self.visible_indices.is_empty() {
            if self.selected_index >= self.visible_indices.len() {
                self.selected_index = self.visible_indices.len() - 1;
            }
        } else {
            self.selected_index = 0;
        }

        // Recalculate visual structure for visible subset
        self.recalculate_visual_structure();
    }

    /// Recalculate depth/is_last/gutters for visible nodes after filtering.
    /// This prevents dangling connectors when middle nodes are filtered out.
    fn recalculate_visual_structure(&mut self) {
        if self.visible_indices.is_empty() {
            return;
        }

        // Build visible node map: entry_id -> (depth, is_last, parent_id)
        let visible_ids: std::collections::HashSet<&str> = self
            .visible_indices
            .iter()
            .map(|&idx| self.items[idx].entry_id.as_str())
            .collect();

        // Find nearest visible ancestor for each visible node (use owned strings)
        let mut visible_parent: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let item_refs: Vec<(String, Option<String>)> = self
            .visible_indices
            .iter()
            .map(|&idx| {
                let item = &self.items[idx];
                (item.entry_id.clone(), item.parent_id.clone())
            })
            .collect();

        for (entry_id, parent_id) in &item_refs {
            let mut current = parent_id.as_deref();
            while let Some(pid) = current {
                if visible_ids.contains(pid) {
                    visible_parent.insert(entry_id.clone(), pid.to_string());
                    break;
                }
                current = self
                    .items
                    .iter()
                    .find(|i| i.entry_id == pid)
                    .and_then(|i| i.parent_id.as_deref());
            }
        }

        // Build visible children map (owned keys)
        let mut visible_children: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();
        for &idx in &self.visible_indices {
            let entry_id = &self.items[idx].entry_id;
            let parent = visible_parent.get(entry_id).cloned().unwrap_or_default();
            visible_children.entry(parent).or_default().push(idx);
        }

        // Sort children by original order
        for (_, children) in visible_children.iter_mut() {
            children.sort();
        }

        // DFS from roots (entries with no visible parent)
        let roots: Vec<String> = self
            .visible_indices
            .iter()
            .filter_map(|&idx| {
                let entry_id = &self.items[idx].entry_id;
                if !visible_parent.contains_key(entry_id) {
                    Some(entry_id.clone())
                } else {
                    None
                }
            })
            .collect();

        let mut stack: Vec<(usize, String, bool, Vec<bool>)> = Vec::new();
        for (i, root) in roots.iter().enumerate() {
            stack.push((0, root.clone(), i == roots.len() - 1, Vec::new()));
        }

        // Recalculate depth/is_last/gutters via DFS
        let mut updates: Vec<(usize, usize, bool, Vec<bool>)> = Vec::new();
        while let Some((depth, entry_id, is_last, gutters)) = stack.pop() {
            if let Some(&idx) = self
                .visible_indices
                .iter()
                .find(|&&i| self.items[i].entry_id == entry_id)
            {
                updates.push((idx, depth, is_last, gutters.clone()));
            }
            if let Some(children) = visible_children.get(&entry_id) {
                let count = children.len();
                for (i, &child_idx) in children.iter().enumerate().rev() {
                    let child_is_last = i == count - 1;
                    let mut child_gutters = gutters.clone();
                    let parent_is_branch = count > 1;
                    child_gutters.push(parent_is_branch && i < count - 1);
                    let child_id = self.items[child_idx].entry_id.clone();
                    stack.push((depth + 1, child_id, child_is_last, child_gutters));
                }
            }
        }
        // Apply updates
        for (idx, depth, is_last, gutters) in updates {
            self.items[idx].depth = depth;
            self.items[idx].is_last = is_last;
            self.items[idx].gutters = gutters;
        }
    }

    pub fn set_filter_mode(&mut self, mode: TreeFilterMode) {
        self.filter_mode = mode;
        self.rebuild_visible();
    }
    pub fn cycle_filter(&mut self) {
        self.filter_mode = match self.filter_mode {
            TreeFilterMode::Default => TreeFilterMode::NoTools,
            TreeFilterMode::NoTools => TreeFilterMode::UserOnly,
            TreeFilterMode::UserOnly => TreeFilterMode::LabeledOnly,
            TreeFilterMode::LabeledOnly => TreeFilterMode::All,
            TreeFilterMode::All => TreeFilterMode::Default,
        };
        self.rebuild_visible();
    }

    pub fn append_search(&mut self, c: char) {
        self.search_query.push(c);
        self.folded_ids.clear();
        self.rebuild_visible();
    }
    pub fn pop_search(&mut self) {
        self.search_query.pop();
        self.folded_ids.clear();
        self.rebuild_visible();
    }
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.folded_ids.clear();
        self.rebuild_visible();
    }
    pub fn toggle_label_timestamps(&mut self) {
        self.show_label_timestamps = !self.show_label_timestamps;
    }

    pub fn start_edit_label(&mut self) {
        if let Some(idx) = self.visible_indices.get(self.selected_index) {
            let item = &self.items[*idx];
            self.edit_label_entry_id = Some(item.entry_id.clone());
            self.edit_label_buffer = item.label.clone().unwrap_or_default();
        }
    }

    pub fn cancel_edit_label(&mut self) {
        self.edit_label_entry_id = None;
        self.edit_label_buffer.clear();
    }
}

impl TuiApp {
    /// Render tree view lines
    pub fn render_tree_view_lines(&self, width: u16) -> Vec<Line<'static>> {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let dim = Style::default().add_modifier(Modifier::DIM);
        let yellow = Style::default().fg(Color::Yellow);
        let green = Style::default().fg(Color::Green);
        let bg_selected = Style::default().bg(Color::Indexed(237));

        if let Some(ref tv) = self.tree_view {
            let mut result: Vec<Line<'static>> = Vec::new();
            result.push(Line::from(Span::styled("Session Tree", bold)));
            if !tv.search_query.is_empty() {
                result.push(Line::from(Span::styled(
                    format!("Search: {}", tv.search_query),
                    dim,
                )));
            }

            for (vi, &item_idx) in tv.visible_indices.iter().enumerate() {
                let item = &tv.items[item_idx];
                let selected = vi == tv.selected_index;
                let mut spans: Vec<Span<'static>> = Vec::new();

                // Cursor
                spans.push(if selected {
                    Span::styled("\u{2192} ", Style::default().fg(Color::Cyan))
                } else {
                    Span::raw("  ")
                });

                // Gutters + connector
                let mut prefix = String::new();
                if !item.gutters.is_empty() {
                    for &g in &item.gutters[..item.gutters.len().saturating_sub(1)] {
                        prefix.push_str(if g { "\u{2502}  " } else { "   " });
                    }
                    if let Some(&last_g) = item.gutters.last() {
                        if item.is_last {
                            prefix.push_str(if last_g { "\u{2514}" } else { " " });
                        } else {
                            prefix.push_str(if last_g { "\u{251c}" } else { " " });
                        }
                        prefix.push('\u{2500}');
                    }
                }

                // Fold indicator: ⊞ folded, ⊟ unfoldable, ─ no children
                if item.has_children {
                    let folded = tv.folded_ids.contains(&item.entry_id);
                    prefix.push_str(if folded { "\u{229e} " } else { "\u{229f} " });
                } else {
                    prefix.push_str("\u{2500} ");
                }
                spans.push(Span::raw(prefix));

                // Active path bullet (•)
                let is_on_active_path = tv.active_path_ids.contains(&item.entry_id)
                    || tv.current_leaf_id.as_deref() == Some(&item.entry_id);
                if is_on_active_path {
                    spans.push(Span::styled("\u{2022} ", green));
                } else {
                    spans.push(Span::raw("  "));
                }

                // Label display [label]
                if let Some(ref lbl) = item.label {
                    spans.push(Span::styled(format!("[{}] ", lbl), yellow));
                }

                // Display label (content)
                let label_max = std::cmp::min(width.saturating_sub(8) as usize, 80);
                let display = if item.display_label.len() > label_max {
                    let max_b = label_max.saturating_sub(3);
                    let mut end = max_b;
                    while !item.display_label.is_char_boundary(end) {
                        end -= 1;
                    }
                    format!("{}...", &item.display_label[..end])
                } else {
                    item.display_label.clone()
                };
                spans.push(Span::raw(display));

                // Label timestamp
                if tv.show_label_timestamps {
                    if let Some(ref ts) = item.label_timestamp {
                        let ts_secs = ts.parse::<i64>().unwrap_or(0) / 1000;
                        let hours = (ts_secs / 3600) % 24;
                        let mins = (ts_secs / 60) % 60;
                        spans.push(Span::styled(format!(" {:02}:{:02}", hours, mins), dim));
                    }
                }

                let line = Line::from(spans);
                result.push(if selected {
                    Line::from(vec![Span::styled(line.to_string(), bg_selected)])
                } else {
                    line
                });
            }

            // Status line
            let mode_label = tv.filter_mode.label();
            let status = format!(
                "  ({}/{}) [{}]{}",
                tv.selected_index + 1,
                tv.visible_count(),
                mode_label,
                if tv.show_label_timestamps {
                    " [+label time]"
                } else {
                    ""
                },
            );
            result.push(Line::from(Span::styled(status, dim)));
            result.push(Line::from(""));
            result.push(Line::from(Span::styled(
                "\u{2191}/\u{2193}: move \u{2190}/\u{2192}: fold Ctrl+D/U/T/L/A: filter Ctrl+O: cycle Enter: select Esc: cancel".to_string(),
                dim,
            )));
            result
        } else {
            vec![]
        }
    }
}
