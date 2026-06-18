//! Subagent tool - delegate tasks to specialized agents

pub mod runner;
pub mod stats;

pub use runner::create_subagent_tool;
pub use runner::create_subagent_tool_with_mode;
