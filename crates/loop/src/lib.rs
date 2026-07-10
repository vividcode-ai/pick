//! Pick Loop — scheduled loop job engine for the Pick AI coding assistant.
//!
//! This crate provides the core loop scheduling functionality, analogous to
//! OpenCode Loop. It supports:
//!
//! - Interval-based loop jobs (including idle-driven)
//! - Goal mode with completion/blocked/progress tools
//! - Preflight, verify, and postrun lifecycle hooks
//! - Git checkpoints and branch management
//! - Watch paths for file-change-triggered execution
//! - Persistence in `.pick/loops/<session>.json`
//!
//! The crate is organized into:
//! - [`types`] — Data model (LoopJob, LoopJobStatus, LoopStore)
//! - [`manager`] — CRUD, state machine, persistence
//! - [`scheduler`] — Async three-layer scheduling engine
//! - [`commands`] — Loop command parser (/loop, /loop-status, ...)
//! - [`integration`] — AgentLoopConfig hook factories
//! - [`goal`] — Goal prompt builder
//! - [`tools`] — Goal tool definitions

pub mod commands;
pub mod goal;
pub mod integration;
pub mod manager;
pub mod scheduler;
pub mod tools;
pub mod types;

// Re-export the most commonly used types.
pub use manager::{LoopManager, load_loop_manager, loops_dir, loops_path_for_session};
pub use scheduler::LoopScheduler;
pub use types::{LoopJob, LoopJobStatus, LoopJobStatusInfo, LoopStore};
