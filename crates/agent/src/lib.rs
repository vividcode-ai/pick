//! Pick-agent: Agent loop, session management, and tool system

// Allow dead code: crate is used as a library by Pick-cli, but compiled separately.
#![allow(dead_code)]
#![allow(ambiguous_glob_reexports)]

pub mod agent_config;
pub mod agent_registry;
pub mod inter_agent;
pub mod permission;
pub mod session;
pub mod tools;
pub mod core;
pub mod extensions;
pub mod skills;
pub mod utils;

pub use session::*;
pub use tools::*;
pub use core::*;
pub use permission::*;
