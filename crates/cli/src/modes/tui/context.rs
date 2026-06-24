use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use tokio::sync::{mpsc, watch};

use crate::args::Args;
use crate::core::agent_mode::AgentMode;
use crate::core::auth_storage::AuthStorage;
use crate::core::resource_loader::ResourceLoader;

use pick_agent::core::events::AgentEvent;
use pick_agent::core::message_queue::{PendingMessageQueue, QueueMode};
use pick_agent::core::state::{AgentTool, ThinkingLevel};
use pick_agent::extensions::runner::ExtensionRunner;
use pick_agent::session::SessionManager;
use pick_ai::types::Message;
use pick_ai::types::Model as AiModel;
use pick_mcp::McpManager;
use pick_tui::app::TuiApp;
use pick_tui::terminal_manager::TerminalManager;
use ratatui::backend::CrosstermBackend;

use super::types::TuiCommand;

/// Shared mutable state for the TUI mode.
/// Fields that are consumed (cmd_rx, evt_rx) remain local in runner.rs.
pub(crate) struct TuiContext {
    // TUI components
    pub tui: TuiApp,
    pub terminal_manager: TerminalManager<CrosstermBackend<std::io::Stdout>>,

    // Channel to agent event callback
    pub cmd_tx: mpsc::UnboundedSender<TuiCommand>,

    // Session and messages
    pub all_messages: Vec<Message>,
    pub session_manager: SessionManager,

    // Model configuration
    pub model: AiModel,
    pub provider: String,
    pub model_id: String,
    pub thinking_level: ThinkingLevel,

    // Agent configuration
    pub tools: Vec<AgentTool>,
    pub system_prompt: String,
    pub agent_mode: AgentMode,
    pub agent_registry: Arc<pick_agent::agent_registry::AgentRegistry>,
    pub resource_loader: ResourceLoader,
    pub extension_runner: Option<Arc<ExtensionRunner>>,

    // MCP
    pub mcp_manager: Arc<McpManager>,
    pub mcp_cancelled: Arc<AtomicBool>,

    // Permissions
    pub permission_manager: Arc<pick_agent::permission::manager::PermissionManager>,
    pub platform_sandbox: Option<Arc<dyn pick_agent::permission::sandbox::Sandbox>>,

    // Auth
    pub auth: Arc<AuthStorage>,

    // OS-level Ctrl+C signal receiver (fallback for Windows edge cases where
    // crossterm raw mode doesn't fully intercept the signal). When the signal
    // fires, the main loop breaks and runs the normal cleanup path.
    pub ctrl_c_rx: watch::Receiver<bool>,

    // Whether to hide thinking blocks (shared with event handler for live updates)
    pub hide_thinking: Arc<AtomicBool>,
    /// Whether to render images inline in terminal
    pub show_images: Arc<AtomicBool>,
    /// Whether to prevent images from being sent to the LLM
    pub block_images: Arc<AtomicBool>,

    // Tool tracking
    pub tool_start_times: Arc<Mutex<HashMap<String, Instant>>>,
    pub tool_args_map: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    pub on_event: Arc<dyn Fn(AgentEvent) + Send + Sync>,

    // Input state
    pub pending_command: Option<String>,
    pub scoped_models: Vec<String>,
    pub was_interrupted: Arc<AtomicBool>,

    // Queues for user messages
    pub steer_queue: Arc<Mutex<PendingMessageQueue>>,
    pub follow_up_queue: Arc<Mutex<PendingMessageQueue>>,
    pub next_turn_queue: Arc<Mutex<PendingMessageQueue>>,
    pub steer_queue_mode: QueueMode,
    pub follow_up_queue_mode: QueueMode,

    // Agent runtime tracking (flat EventStream model)
    pub agent_is_running: bool,
    pub agent_cancel_tx: Option<std::sync::Arc<tokio::sync::watch::Sender<bool>>>,
    pub agent_cancel_requested: std::sync::Arc<AtomicBool>,
    pub agent_abort_handle: Option<tokio::task::AbortHandle>,
    pub agent_start_message_count: usize,

    // CLI args
    pub args: Args,
    pub all_tools: Arc<RwLock<Vec<AgentTool>>>,

    // Environment
    pub cwd: std::path::PathBuf,
    pub version: &'static str,
    pub app_name: &'static str,

    // Update action to execute after TUI exits
    pub pending_update: Option<crate::core::update_action::UpdateAction>,
}
