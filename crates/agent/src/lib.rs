//! Pick-agent: Agent loop, session management, and tool system

// Allow dead code: crate is used as a library by Pick-cli, but compiled separately.
#![allow(dead_code)]
#![allow(ambiguous_glob_reexports)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::doc_lazy_continuation)]
#![allow(clippy::should_implement_trait)]
#![allow(clippy::unnecessary_get_then_check)]
#![allow(clippy::match_like_matches_macro)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::unnecessary_unwrap)]

pub mod agent_config;
pub mod agent_registry;
pub mod core;
pub mod extensions;
pub mod inter_agent;
pub mod permission;
pub mod prompt_history;
pub mod session;
pub mod settings;
pub mod skills;
pub mod tools;
pub mod utils;

pub use core::*;
pub use permission::*;
pub use session::*;
pub use tools::*;
