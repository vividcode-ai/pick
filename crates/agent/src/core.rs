//! Agent core module

pub mod agent_loop;
pub mod compaction;
pub mod config;
pub mod diagnostics;
pub mod events;
pub mod hooks;
pub mod message_queue;
pub mod state;

pub use agent_loop::*;
pub use config::*;
pub use events::*;
pub use message_queue::*;
pub use state::*;
