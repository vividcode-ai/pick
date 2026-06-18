//! Ring buffer for Emacs-style kill/yank operations

/// Ring buffer for killed (deleted) text entries.
/// Consecutive kills can accumulate into a single entry.
/// Supports yank (paste most recent) and yank-pop (cycle through older entries).
#[derive(Clone)]
pub struct KillRing {
    ring: Vec<String>,
}

impl KillRing {
    pub fn new() -> Self {
        Self { ring: Vec::new() }
    }

    /// Add text to the kill ring.
    ///
    /// * `text` - The killed text to add
    /// * `prepend` - If accumulating, prepend (backward deletion) or append (forward deletion)
    /// * `accumulate` - Merge with the most recent entry instead of creating a new one
    pub fn push(&mut self, text: &str, prepend: bool, accumulate: bool) {
        if text.is_empty() {
            return;
        }

        if accumulate && !self.ring.is_empty() {
            let last = self.ring.pop().unwrap();
            if prepend {
                self.ring.push(text.to_string() + &last);
            } else {
                self.ring.push(last + text);
            }
        } else {
            self.ring.push(text.to_string());
        }
    }

    /// Get most recent entry without modifying the ring.
    pub fn peek(&self) -> Option<&str> {
        self.ring.last().map(|s| s.as_str())
    }

    /// Move last entry to front (for yank-pop cycling).
    pub fn rotate(&mut self) {
        if self.ring.len() > 1 {
            let last = self.ring.pop().unwrap();
            self.ring.insert(0, last);
        }
    }

    pub fn len(&self) -> usize {
        self.ring.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }
}

impl Default for KillRing {
    fn default() -> Self {
        Self::new()
    }
}
