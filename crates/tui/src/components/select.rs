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

/// A select list component with pagination support
pub struct SelectList {
    pub items: Vec<SelectItem>,
    pub selected_index: usize,
    pub title: String,
    pub page: usize,
    pub page_size: usize,
}

impl SelectList {
    pub fn new(title: impl Into<String>, items: Vec<SelectItem>) -> Self {
        Self {
            items,
            selected_index: 0,
            title: title.into(),
            page: 0,
            page_size: 7,
        }
    }

    pub fn selected(&self) -> Option<&SelectItem> {
        self.items.get(self.selected_index)
    }

    /// Number of pages given page_size
    pub fn page_count(&self) -> usize {
        if self.items.is_empty() {
            return 1;
        }
        (self.items.len() + self.page_size - 1) / self.page_size
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
}
