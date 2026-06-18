//! TUI component types — only actively used components
//! Removed dead code: box_, text, spacer, loader, cancellable_loader, input, editor_component
//! These were replaced by ratatui's native capabilities (Block, Layout, Paragraph, etc.)

pub mod chat;
pub mod image_component;
pub mod select;
pub mod settings_list;
pub mod theme;
pub mod truncated_text;
pub mod question;
pub mod update_prompt;

pub use chat::*;
pub use image_component::*;
pub use select::*;
pub use settings_list::*;
pub use theme::*;
pub use truncated_text::*;
pub use question::*;
pub use update_prompt::*;
