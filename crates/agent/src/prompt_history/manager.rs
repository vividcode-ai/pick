//! PromptHistoryManager — persistent, sliding-window input history.
//!
//! Stores history in `.pick/prompt-history.jsonl` (one JSON-encoded string per line).
//! Maintains an in-memory sliding window of up to [`WINDOW_SIZE`] entries.
//! The window is loaded lazily on first access and slides to keep the
//! current navigation cursor near the center.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::traits::HistoryProvider;

/// Maximum entries kept in memory at once.
const WINDOW_SIZE: usize = 11;

/// The "center" index where the cursor sits during balanced sliding.
/// When cursor is at this position, further navigation in either direction
/// slides the window (loads one more from the file, drops the opposite side)
/// so that the cursor effectively stays centred with 5 entries visible on
/// each side.
const CENTER: usize = 5;

// ---------------------------------------------------------------------------
// PromptHistoryManager
// ---------------------------------------------------------------------------

pub struct PromptHistoryManager {
    file_path: PathBuf,
    /// Sliding window of history entries (newest at the end).
    window: Vec<String>,
    /// The line index in the file that corresponds to `window[0]`.
    file_start: usize,
    /// Total number of entries in the file (updated on push/reload).
    file_total: usize,
    /// Current navigation cursor: `None` = not browsing.
    cursor: Option<usize>,
    /// Saved user input when entering browse mode.
    staging: String,
    loaded: bool,
}

impl PromptHistoryManager {
    /// Create a new manager for the given project directory.
    /// The file path will be `{project_dir}/.pick/prompt-history.jsonl`.
    /// No I/O is performed until the first access (`load` or `previous`/`next`/`push`).
    pub fn new(project_dir: &Path) -> Self {
        Self {
            file_path: project_dir.join(".pick").join("prompt-history.jsonl"),
            window: Vec::new(),
            file_start: 0,
            file_total: 0,
            cursor: None,
            staging: String::new(),
            loaded: false,
        }
    }

    /// Explicitly load history from the file.  Safe to call multiple times
    /// (second call is a no-op).  Also called lazily by navigation methods.
    pub fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let entries = read_all_entries(&self.file_path);
        self.file_total = entries.len();

        if entries.is_empty() {
            self.window.clear();
            self.file_start = 0;
            return;
        }

        // Take the last WINDOW_SIZE entries
        let start = if entries.len() > WINDOW_SIZE {
            entries.len() - WINDOW_SIZE
        } else {
            0
        };
        self.window = entries[start..].to_vec();
        self.file_start = start;
    }

    fn ensure_loaded(&mut self) {
        if !self.loaded {
            self.load();
        }
    }

    // -- private helpers ----------------------------------------------------

    /// Returns `true` when there are entries in the file that are older than
    /// the current window.
    fn can_slide_older(&self) -> bool {
        self.file_start > 0
    }

    /// Returns `true` when there are entries in the file that are newer than
    /// the current window.
    fn can_slide_newer(&self) -> bool {
        self.file_start + self.window.len() < self.file_total
    }

    /// Slide the window one entry older: prepend the preceding file entry,
    /// drop the newest entry in the window.
    fn slide_older(&mut self) {
        if !self.can_slide_older() {
            return;
        }
        let idx = self.file_start.wrapping_sub(1);
        if let Some(entry) = read_entry_at(&self.file_path, idx) {
            self.window.insert(0, entry);
            self.window.truncate(WINDOW_SIZE);
            self.file_start = idx;
        }
    }

    /// Slide the window one entry newer: append the next file entry,
    /// drop the oldest entry in the window.
    fn slide_newer(&mut self) {
        let idx = self.file_start + self.window.len();
        if idx >= self.file_total {
            return;
        }
        if let Some(entry) = read_entry_at(&self.file_path, idx) {
            self.window.push(entry);
            if self.window.len() > WINDOW_SIZE {
                self.window.remove(0);
                self.file_start += 1;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// HistoryProvider impl
// ---------------------------------------------------------------------------

impl HistoryProvider for PromptHistoryManager {
    fn push(&mut self, text: &str) {
        self.ensure_loaded();
        let trimmed = text.trim().to_string();
        if trimmed.is_empty() {
            return;
        }
        // Deduplicate against the most recent entry
        if self.window.last().map(|s| s.as_str()) == Some(trimmed.as_str()) {
            // Same as the latest entry — skip storing but still exit browse
            // so the user's subsequent ↑ press lands here again.
            self.cursor = None;
            self.staging.clear();
            return;
        }

        // Append to file
        append_entry(&self.file_path, &trimmed);
        self.file_total += 1;

        // Update in-memory window
        if self.window.len() < WINDOW_SIZE {
            self.window.push(trimmed);
        } else {
            self.window.push(trimmed);
            self.window.remove(0);
            self.file_start += 1;
        }

        // Exit browsing
        self.cursor = None;
        self.staging.clear();
    }

    fn previous(&mut self, current_input: &str) -> Option<String> {
        self.ensure_loaded();
        if self.window.is_empty() {
            return None;
        }

        // Entering browse mode
        if self.cursor.is_none() {
            self.staging = current_input.to_string();
            self.cursor = Some(self.window.len() - 1);
            return Some(self.window[self.window.len() - 1].clone());
        }

        let cursor = self.cursor.unwrap();

        // Already at the oldest entry
        if cursor == 0 {
            return None;
        }

        // At center — slide the window instead of moving the cursor
        if cursor == CENTER && self.can_slide_older() {
            self.slide_older();
            // cursor stays at CENTER, but the entry at window[CENTER] is now
            // the physical entry that was one position older
            return Some(self.window[CENTER].clone());
        }

        // Normal backward navigation
        let new = cursor - 1;
        self.cursor = Some(new);
        Some(self.window[new].clone())
    }

    fn next(&mut self, _current_input: &str) -> Option<String> {
        self.ensure_loaded();

        let cursor = self.cursor?; // not browsing → None

        // At the newest entry → exit browsing and restore staging
        if cursor == self.window.len().wrapping_sub(1) {
            let result = if self.staging.is_empty() {
                String::new()
            } else {
                self.staging.clone()
            };
            self.cursor = None;
            self.staging.clear();
            return Some(result);
        }

        // At center — slide the window instead of moving the cursor
        if cursor == CENTER && self.can_slide_newer() {
            self.slide_newer();
            return Some(self.window[CENTER].clone());
        }

        // Normal forward navigation
        let new = cursor + 1;
        self.cursor = Some(new);
        Some(self.window[new].clone())
    }

    fn window(&self) -> &[String] {
        &self.window
    }

    fn reset(&mut self) {
        self.cursor = None;
        self.staging.clear();
    }

    fn is_browsing(&self) -> bool {
        self.cursor.is_some()
    }
}

// ---------------------------------------------------------------------------
// File I/O helpers
// ---------------------------------------------------------------------------

/// Read all entries from a JSONL file, returning them in file order
/// (oldest first).
fn read_all_entries(path: &Path) -> Vec<String> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let reader = std::io::BufReader::new(file);
    let stream = serde_json::Deserializer::from_reader(reader).into_iter::<String>();
    stream.filter_map(|r| r.ok()).collect()
}

/// Read a single entry at the given line index (0-based) from the file.
fn read_entry_at(path: &Path, index: usize) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);
    let stream = serde_json::Deserializer::from_reader(reader).into_iter::<String>();
    stream.filter_map(|r| r.ok()).nth(index)
}

/// Append a single entry (JSON-encoded) to the JSONL file.
fn append_entry(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(f) => f,
        Err(_) => return,
    };
    let line = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string());
    let _ = writeln!(file, "{}", line);
}
