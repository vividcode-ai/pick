//! Settings list component with keyboard navigation and submenus

use crate::fuzzy::fuzzy_filter;
use crate::utils::{truncate_to_width, visible_width};

/// A single setting item
#[derive(Clone)]
pub struct SettingItem {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub current_value: String,
    pub values: Option<Vec<String>>,
}

/// Settings list component
pub struct SettingsList {
    items: Vec<SettingItem>,
    filtered_items: Vec<usize>,
    selected_index: usize,
    max_visible: usize,
}

impl SettingsList {
    pub fn new(items: Vec<SettingItem>, max_visible: usize) -> Self {
        let count = items.len();
        Self {
            filtered_items: (0..count).collect(),
            items,
            selected_index: 0,
            max_visible,
        }
    }

    pub fn update_value(&mut self, id: &str, new_value: String) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.current_value = new_value;
        }
    }

    pub fn selected_item(&self) -> Option<&SettingItem> {
        let idx = self.filtered_items.get(self.selected_index)?;
        self.items.get(*idx)
    }

    pub fn selected_item_mut(&mut self) -> Option<&mut SettingItem> {
        let idx = *self.filtered_items.get(self.selected_index)?;
        self.items.get_mut(idx)
    }

    pub fn handle_input(&mut self, data: &str) -> bool {
        match data {
            "up" | "ctrl+p" => {
                if !self.filtered_items.is_empty() {
                    self.selected_index = if self.selected_index == 0 {
                        self.filtered_items.len() - 1
                    } else {
                        self.selected_index - 1
                    };
                }
                true
            }
            "down" | "ctrl+n" => {
                if !self.filtered_items.is_empty() {
                    self.selected_index = if self.selected_index == self.filtered_items.len() - 1 {
                        0
                    } else {
                        self.selected_index + 1
                    };
                }
                true
            }
            "enter" | " " => {
                self.activate_item();
                true
            }
            _ => false,
        }
    }

    pub fn set_search(&mut self, query: &str) {
        if query.is_empty() {
            self.filtered_items = (0..self.items.len()).collect();
        } else {
            self.filtered_items = fuzzy_filter(&self.items, query, |item| &item.label)
                .iter()
                .filter_map(|item| self.items.iter().position(|i| i.id == item.id))
                .collect();
        }
        self.selected_index = 0;
    }

    fn activate_item(&mut self) {
        let item = match self.selected_item_mut() {
            Some(i) => i,
            None => return,
        };

        if let Some(ref values) = item.values.clone() {
            if !values.is_empty() {
                let current_idx = values.iter().position(|v| v == &item.current_value);
                let next_idx = match current_idx {
                    Some(idx) => (idx + 1) % values.len(),
                    None => 0,
                };
                item.current_value = values[next_idx].clone();
            }
        }
    }

    pub fn render(&self, width: usize) -> Vec<String> {
        let mut lines = Vec::new();

        if self.items.is_empty() {
            lines.push("  No settings available".to_string());
            return lines;
        }

        let display_indices = &self.filtered_items;
        if display_indices.is_empty() {
            return lines;
        }

        // Calculate visible range
        let total = display_indices.len();
        let start = if total > self.max_visible {
            let half = self.max_visible / 2;
            if self.selected_index > half {
                std::cmp::min(
                    self.selected_index.saturating_sub(half),
                    total - self.max_visible,
                )
            } else {
                0
            }
        } else {
            0
        };
        let end = std::cmp::min(start + self.max_visible, total);

        // Calculate max label width
        let max_label_width = std::cmp::min(
            30,
            self.items
                .iter()
                .map(|item| visible_width(&item.label))
                .max()
                .unwrap_or(0),
        );

        for i in start..end {
            let item_idx = display_indices[i];
            let item = &self.items[item_idx];
            let is_selected = i == self.selected_index;

            let cursor = if is_selected { "› " } else { "  " };

            let label_padded = format!(
                "{}{}",
                item.label,
                " ".repeat(max_label_width.saturating_sub(visible_width(&item.label)))
            );

            let separator = "  ";
            let used_width = 2 + max_label_width + visible_width(separator);
            let value_max_width = width.saturating_sub(used_width + 2);
            let value_text = truncate_to_width(&item.current_value, value_max_width);

            let line = format!("{}{}{}[{}]", cursor, label_padded, separator, value_text);
            lines.push(truncate_to_width(&line, width));
        }

        // Scroll indicator
        if total > self.max_visible {
            lines.push(format!("  ({}/{})", self.selected_index + 1, total));
        }

        // Description for selected item
        if let Some(item) = self.selected_item() {
            if let Some(ref desc) = item.description {
                lines.push(String::new());
                lines.push(format!("  {}", desc));
            }
        }

        lines.push(String::new());
        lines.push("  ↑↓: navigate · Enter/Space: toggle".to_string());

        lines
    }
}
