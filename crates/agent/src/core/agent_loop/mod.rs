//! Agent loop - the core execution loop for the AI agent

pub mod runner;
pub mod stream;
pub mod tools;

pub use runner::*;
pub use stream::*;
pub use tools::*;

use std::pin::Pin;
use std::sync::Arc;

use pick_ai::types::Model;

use super::events::AgentEventHandler;
use super::state::{AgentTool, ApproveFn, QuestionFn, ThinkingLevel};
use crate::agent_registry::AgentRegistry;
use crate::extensions::runner::ExtensionRunner;
use crate::permission::Ruleset;
use crate::permission::fs_policy::FileSystemPolicy;
use crate::permission::manager::PermissionManager;

/// Maximum consecutive tool errors before forcing text-only mode.
pub const MAX_CONSECUTIVE_TOOL_ERRORS: u32 = 10;

/// Moderate consecutive tool errors threshold for plan-aware recovery injection.
/// When crossed, a system message is injected encouraging the LLM to check
/// the todo_plan and skip to the next step rather than repeating the same failure.
pub const PLAN_RECOVERY_THRESHOLD: u32 = 3;

/// Configuration for the agent loop
#[derive(Clone)]
pub struct AgentLoopConfig {
    pub model: Model,
    pub system_prompt: String,
    pub tools: Vec<AgentTool>,
    pub thinking_level: ThinkingLevel,
    pub max_tokens: Option<u64>,
    pub temperature: Option<f64>,
    pub on_event: Option<AgentEventHandler>,
    pub approve: Option<ApproveFn>,
    pub question: Option<QuestionFn>,
    pub agent_id: Option<String>,
    pub agent_registry: Option<Arc<AgentRegistry>>,
    pub extension_runner: Option<Arc<ExtensionRunner>>,
    /// Permission hook registry for pre/post-tool-use interceptors
    pub permission_hooks: Option<Arc<crate::permission::hooks::PermissionHookRegistry>>,
    /// Mode rulesets for automatic Allow/Deny decisions before prompting user
    pub mode_rulesets: Option<Vec<Ruleset>>,
    // === Hooks ===
    /// Hook to provide steering messages before each turn
    pub get_steering_messages: Option<Arc<dyn Fn() -> Vec<pick_ai::types::Message> + Send + Sync>>,
    /// Hook to modify context before LLM call
    pub transform_context:
        Option<Arc<dyn Fn(pick_ai::types::Context) -> pick_ai::types::Context + Send + Sync>>,
    /// Hook to validate tool calls before execution (return Some(error) to block)
    pub before_tool_call:
        Option<Arc<dyn Fn(&pick_ai::types::ToolCall) -> Option<String> + Send + Sync>>,
    /// Hook to determine if agent should stop after a turn
    pub should_stop_after_turn:
        Option<Arc<dyn Fn(&pick_ai::types::AssistantMessage) -> bool + Send + Sync>>,
    /// Hook to provide follow-up messages after agent completes
    pub get_follow_up_messages:
        Option<Arc<dyn Fn(&AgentRunResult) -> Vec<pick_ai::types::Message> + Send + Sync>>,
    /// Hook to provide dynamic API key
    pub get_api_key: Option<Arc<dyn Fn() -> Option<String> + Send + Sync>>,
    /// Hook called after each turn completes, providing current messages
    /// for incremental session persistence. If this returns an error, it is logged
    /// but does not interrupt the agent loop.
    pub on_turn_complete: Option<
        Arc<
            dyn Fn(&[pick_ai::types::Message]) -> Pin<Box<dyn Send + Future<Output = ()>>>
                + Send
                + Sync,
        >,
    >,
    /// Maximum provider-level HTTP retry attempts (default: 3)
    pub provider_max_retries: Option<u32>,
    /// Maximum provider-level retry delay in ms (default: 60000)
    pub provider_max_retry_delay_ms: Option<u64>,
    /// File system policy for path sandboxing
    pub fs_policy: Option<Arc<FileSystemPolicy>>,
    /// Current working directory for path resolution
    pub cwd: Option<std::path::PathBuf>,
    /// Permission manager orchestrating all permission layers
    pub permission_manager: Option<Arc<PermissionManager>>,
    /// Platform sandbox for process isolation (bwrap, seatbelt, restricted token)
    pub sandbox: Option<Arc<dyn crate::permission::sandbox::Sandbox>>,
}

/// Result from a single agent run
pub struct AgentRunResult {
    pub messages: Vec<pick_ai::types::Message>,
    pub usage: pick_ai::types::Usage,
}
