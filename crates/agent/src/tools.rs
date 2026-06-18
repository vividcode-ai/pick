//! Agent tool implementations

pub mod read;
pub mod write;
pub mod edit;
pub mod bash;
pub mod grep;
pub mod find;
pub mod ls;
pub mod registry;
pub mod subagent;

pub mod webfetch;
pub mod todo_plan;
pub mod question;
pub mod goal;

pub use registry::*;
