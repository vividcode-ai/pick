//! Select list component for choosing from options

/// An item in a select list
#[derive(Clone)]
pub struct SelectItem {
    pub label: String,
    pub value: String,
    pub description: Option<String>,
    pub disabled: bool,
}

impl SelectItem {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            description: None,
            disabled: false,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// A select list component with pagination and search support
pub struct SelectList {
    pub items: Vec<SelectItem>,
    pub selected_index: usize,
    pub title: String,
    pub page: usize,
    pub page_size: usize,
    /// Original unfiltered items (kept when search query is active)
    all_items: Vec<SelectItem>,
    /// Current search/filter query
    pub search_query: String,
    /// Informational lines shown after the title in the popup (dimmed, not selectable)
    pub info_lines: Vec<String>,
}

impl SelectList {
    pub fn new(title: impl Into<String>, items: Vec<SelectItem>) -> Self {
        Self {
            items: items.clone(),
            selected_index: 0,
            title: title.into(),
            page: 0,
            page_size: 7,
            all_items: items,
            search_query: String::new(),
            info_lines: Vec::new(),
        }
    }

    pub fn with_info_lines(mut self, lines: Vec<String>) -> Self {
        self.info_lines = lines;
        self
    }

    pub fn selected(&self) -> Option<&SelectItem> {
        self.items.get(self.selected_index)
    }

    /// Number of pages given page_size
    pub fn page_count(&self) -> usize {
        if self.items.is_empty() {
            return 1;
        }
        self.items.len().div_ceil(self.page_size)
    }

    /// Global start index of the current page
    pub fn page_start(&self) -> usize {
        self.page * self.page_size
    }

    /// Global end index (exclusive) of the current page
    pub fn page_end(&self) -> usize {
        std::cmp::min(self.page_start() + self.page_size, self.items.len())
    }

    /// Move selection to next item (no wrap)
    pub fn next(&mut self) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        if self.selected_index + 1 < self.page_end() {
            self.selected_index += 1;
        } else if self.page + 1 < self.page_count() {
            self.page += 1;
            self.selected_index = self.page * self.page_size;
        }
    }

    /// Move selection to previous item (no wrap)
    pub fn previous(&mut self) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        let page_start = self.page * self.page_size;
        if self.selected_index > page_start {
            self.selected_index -= 1;
        } else if self.page > 0 {
            self.page -= 1;
            self.selected_index = std::cmp::min((self.page + 1) * self.page_size - 1, len - 1);
        }
    }

    /// Go to next page
    pub fn next_page(&mut self) {
        if self.items.is_empty() {
            return;
        }
        if self.page + 1 < self.page_count() {
            self.page += 1;
            self.selected_index = self.page * self.page_size;
        }
    }

    /// Go to previous page
    pub fn prev_page(&mut self) {
        if self.page > 0 {
            self.page -= 1;
            self.selected_index = self.page * self.page_size;
        }
    }

    /// Jump to first item
    pub fn first(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.page = 0;
        self.selected_index = 0;
    }

    /// Jump to last item
    pub fn last(&mut self) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        self.page = self.page_count() - 1;
        self.selected_index = len - 1;
    }

    /// Re-filter `items` from `all_items` based on `search_query`.
    fn apply_search(&mut self) {
        if self.search_query.is_empty() {
            self.items = self.all_items.clone();
        } else {
            let q = self.search_query.to_lowercase();
            self.items = self
                .all_items
                .iter()
                .filter(|item| {
                    item.label.to_lowercase().contains(&q)
                        || item.value.to_lowercase().contains(&q)
                        || item
                            .description
                            .as_ref()
                            .is_some_and(|d| d.to_lowercase().contains(&q))
                })
                .cloned()
                .collect();
        }
        self.selected_index = 0;
        self.page = 0;
    }

    /// Append a character to the search query and re-filter.
    pub fn push_search_char(&mut self, c: char) {
        self.search_query.push(c);
        self.apply_search();
    }

    /// Remove the last character from the search query and re-filter.
    pub fn pop_search_char(&mut self) {
        self.search_query.pop();
        self.apply_search();
    }

    /// Clear the search query and restore all items.
    pub fn clear_search(&mut self) {
        if !self.search_query.is_empty() {
            self.search_query.clear();
            self.apply_search();
        }
    }

    /// Whether a search query is currently active.
    pub fn has_search(&self) -> bool {
        !self.search_query.is_empty()
    }
}
