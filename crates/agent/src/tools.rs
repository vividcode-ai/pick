//! Agent tool implementations

pub mod bash;
pub mod edit;
pub mod find;
pub mod grep;
pub mod ls;
pub mod read;
pub mod registry;
pub mod subagent;
pub mod write;

pub mod goal;
pub mod question;
pub mod todo_plan;
pub mod webfetch;

pub use registry::*;
