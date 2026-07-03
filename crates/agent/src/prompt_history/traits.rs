//! HistoryProvider trait — shared interface for prompt history navigation.
//! Both TUI (pick-tui) and Web server (pick-server) consume this trait.

/// Trait for prompt history navigation.
/// Implementations handle persistence, deduplication, window management,
/// and navigation cursor state.
pub trait HistoryProvider: Send {
    /// Append a submitted prompt to history (deduplicates, persists).
    fn push(&mut self, text: &str);

    /// Navigate backward (older) in history.
    /// - If not yet browsing: saves `current_input` as staging, points to newest entry.
    /// - If already browsing: returns the previous (older) entry.
    /// Returns `None` when already at the oldest entry.
    fn previous(&mut self, current_input: &str) -> Option<String>;

    /// Navigate forward (newer) in history.
    /// - If not browsing: returns `None`.
    /// - If at the newest entry: exits browsing mode and returns the staging text.
    /// - Otherwise: returns the next (newer) entry.
    fn next(&mut self, current_input: &str) -> Option<String>;

    /// Current in-memory window of history entries (newest at end).
    fn window(&self) -> &[String];

    /// Exit browsing mode without committing.
    fn reset(&mut self);

    /// Whether the user is currently browsing history entries.
    fn is_browsing(&self) -> bool;
}
