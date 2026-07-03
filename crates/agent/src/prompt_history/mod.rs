//! Shared prompt history management with sliding-window lazy loading.
//!
//! Provides [`HistoryProvider`] trait and [`PromptHistoryManager`]
//! implementation that persists to `.pick/prompt-history.jsonl`.

pub mod manager;
pub mod traits;

pub use manager::PromptHistoryManager;
pub use traits::HistoryProvider;
