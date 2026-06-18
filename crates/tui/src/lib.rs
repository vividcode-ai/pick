//! Pick-tui: Terminal UI components and rendering
#![allow(dead_code)]

// Core rendering infrastructure
pub mod custom_terminal;
pub(crate) mod insert_history;
pub(crate) mod live_wrap;
pub(crate) mod markdown_render;
pub mod terminal_manager;
pub(crate) mod wrapping;

// Existing Pick modules (updated to use ratatui)
pub mod app;
pub mod autocomplete;
pub mod components;
pub mod editor;
pub mod fuzzy;
pub mod keys;
pub mod kill_ring;
pub mod native_modifiers;
pub mod stdin_buffer;
pub mod terminal;
pub mod terminal_image;
pub mod undo_stack;
pub mod keybindings;
pub mod paste_burst;
pub mod syntax_highlight;
pub mod utils;

// Re-exports
pub use app::*;
pub use autocomplete::*;
pub use editor::*;
pub use fuzzy::*;
pub use keys::*;
pub use kill_ring::*;
pub use native_modifiers::*;
pub use stdin_buffer::*;
pub use terminal::*;
pub use terminal_image::*;
pub use undo_stack::*;
pub use keybindings::*;
pub use utils::*;
