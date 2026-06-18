//! Extension type definitions

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

// ============================================================================
// Resource Events
// ============================================================================

#[derive(Debug, Clone)]
pub struct ResourcesDiscoverEvent {
    pub cwd: String,
    pub reason: ResourcesDiscoverReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourcesDiscoverReason {
    Startup,
    Reload,
}

#[derive(Debug, Clone, Default)]
pub struct ResourcesDiscoverResult {
    pub skill_paths: Vec<String>,
    pub prompt_paths: Vec<String>,
    pub theme_paths: Vec<String>,
}

// ============================================================================
// Session Events
// ============================================================================

#[derive(Debug, Clone)]
pub struct SessionBeforeCompactEvent {
    pub preparation: serde_json::Value,
    pub branch_entries: Vec<serde_json::Value>,
    pub custom_instructions: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SessionCompactEvent {
    pub compaction_entry: serde_json::Value,
    pub from_extension: bool,
}

#[derive(Debug, Clone)]
pub struct TreePreparation {
    pub target_id: String,
    pub old_leaf_id: Option<String>,
    pub common_ancestor_id: Option<String>,
    pub entries_to_summarize: Vec<serde_json::Value>,
    pub user_wants_summary: bool,
    pub custom_instructions: Option<String>,
    pub replace_instructions: Option<bool>,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SessionBeforeTreeEvent {
    pub preparation: TreePreparation,
}

#[derive(Debug, Clone)]
pub struct SessionTreeEvent {
    pub new_leaf_id: Option<String>,
    pub old_leaf_id: Option<String>,
    pub summary_entry: Option<serde_json::Value>,
    pub from_extension: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct SessionBeforeCompactResult {
    pub cancel: Option<bool>,
    pub compaction: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct SessionBeforeTreeResult {
    pub cancel: Option<bool>,
    pub summary: Option<serde_json::Value>,
    pub custom_instructions: Option<String>,
    pub replace_instructions: Option<bool>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStartReason {
    Startup,
    Reload,
    New,
    Resume,
    Fork,
}

#[derive(Debug, Clone)]
pub struct SessionStartEvent {
    pub reason: SessionStartReason,
    pub previous_session_file: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SessionBeforeSwitchEvent {
    pub reason: SessionBeforeSwitchReason,
    pub target_session_file: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionBeforeSwitchReason {
    New,
    Resume,
}

#[derive(Debug, Clone)]
pub struct SessionBeforeForkEvent {
    pub entry_id: String,
    pub position: ForkPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForkPosition {
    Before,
    At,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionShutdownReason {
    Quit,
    Reload,
    New,
    Resume,
    Fork,
}

#[derive(Debug, Clone)]
pub struct SessionShutdownEvent {
    pub reason: SessionShutdownReason,
    pub target_session_file: Option<String>,
}

// ============================================================================
// Agent Events
// ============================================================================

#[derive(Debug, Clone)]
pub struct ContextEvent {
    pub messages: Vec<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct BeforeProviderRequestEvent {
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct AfterProviderResponseEvent {
    pub status: u16,
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct BeforeAgentStartEvent {
    pub prompt: String,
    pub system_prompt: String,
}

#[derive(Debug, Clone)]
pub struct AgentStartEvent;

#[derive(Debug, Clone)]
pub struct AgentEndEvent {
    pub messages: Vec<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct TurnStartEvent {
    pub turn_index: usize,
    pub timestamp: i64,
}

#[derive(Debug, Clone)]
pub struct TurnEndEvent {
    pub turn_index: usize,
}

#[derive(Debug, Clone)]
pub struct MessageStartEvent {
    pub message: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct MessageUpdateEvent {
    pub message: serde_json::Value,
    pub assistant_message_event: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct MessageEndEvent {
    pub message: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ToolExecutionStartEvent {
    pub tool_call_id: String,
    pub tool_name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ToolExecutionUpdateEvent {
    pub tool_call_id: String,
    pub tool_name: String,
    pub args: serde_json::Value,
    pub partial_result: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ToolExecutionEndEvent {
    pub tool_call_id: String,
    pub tool_name: String,
    pub result: serde_json::Value,
    pub is_error: bool,
}

// ============================================================================
// Model Events
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSelectSource {
    Set,
    Cycle,
    Restore,
}

#[derive(Debug, Clone)]
pub struct ModelSelectEvent {
    pub model: String,
    pub previous_model: Option<String>,
    pub source: ModelSelectSource,
}

#[derive(Debug, Clone)]
pub struct ThinkingLevelSelectEvent {
    pub level: String,
    pub previous_level: String,
}

// ============================================================================
// Input Events
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputSource {
    Interactive,
    Rpc,
    Extension,
}

#[derive(Debug, Clone)]
pub struct InputEvent {
    pub text: String,
    pub source: InputSource,
}

#[derive(Debug, Clone)]
pub enum InputEventResult {
    Continue,
    Transform { text: String },
    Handled,
}

// ============================================================================
// User Bash Events
// ============================================================================

#[derive(Debug, Clone)]
pub struct UserBashEvent {
    pub command: String,
    pub exclude_from_context: bool,
    pub cwd: String,
}

// ============================================================================
// Tool Call Events
// ============================================================================

#[derive(Debug, Clone)]
pub struct ToolCallEvent {
    pub tool_call_id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolResultEvent {
    pub tool_call_id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
    pub content: Vec<serde_json::Value>,
    pub is_error: bool,
}

// ============================================================================
// Extension Event Enum
// ============================================================================

/// Extension event type - dispatched to subscribed extension handlers
#[derive(Debug, Clone)]
pub enum ExtensionEvent {
    ResourcesDiscover(ResourcesDiscoverEvent),
    SessionStart(SessionStartEvent),
    SessionBeforeSwitch(SessionBeforeSwitchEvent),
    SessionBeforeFork(SessionBeforeForkEvent),
    SessionShutdown(SessionShutdownEvent),
    Context(ContextEvent),
    BeforeProviderRequest(BeforeProviderRequestEvent),
    AfterProviderResponse(AfterProviderResponseEvent),
    BeforeAgentStart(BeforeAgentStartEvent),
    AgentStart(AgentStartEvent),
    AgentEnd(AgentEndEvent),
    TurnStart(TurnStartEvent),
    TurnEnd(TurnEndEvent),
    MessageStart(MessageStartEvent),
    MessageUpdate(MessageUpdateEvent),
    MessageEnd(MessageEndEvent),
    ToolExecutionStart(ToolExecutionStartEvent),
    ToolExecutionUpdate(ToolExecutionUpdateEvent),
    ToolExecutionEnd(ToolExecutionEndEvent),
    ModelSelect(ModelSelectEvent),
    ThinkingLevelSelect(ThinkingLevelSelectEvent),
    UserBash(UserBashEvent),
    Input(InputEvent),
    ToolCall(ToolCallEvent),
    ToolResult(ToolResultEvent),
    SessionBeforeCompact(SessionBeforeCompactEvent),
    SessionCompact(SessionCompactEvent),
    SessionBeforeTree(SessionBeforeTreeEvent),
    SessionTree(SessionTreeEvent),
}

impl ExtensionEvent {
    /// Get the event type string identifier
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::ResourcesDiscover(_) => "resources_discover",
            Self::SessionStart(_) => "session_start",
            Self::SessionBeforeSwitch(_) => "session_before_switch",
            Self::SessionBeforeFork(_) => "session_before_fork",
            Self::SessionShutdown(_) => "session_shutdown",
            Self::Context(_) => "context",
            Self::BeforeProviderRequest(_) => "before_provider_request",
            Self::AfterProviderResponse(_) => "after_provider_response",
            Self::BeforeAgentStart(_) => "before_agent_start",
            Self::AgentStart(_) => "agent_start",
            Self::AgentEnd(_) => "agent_end",
            Self::TurnStart(_) => "turn_start",
            Self::TurnEnd(_) => "turn_end",
            Self::MessageStart(_) => "message_start",
            Self::MessageUpdate(_) => "message_update",
            Self::MessageEnd(_) => "message_end",
            Self::ToolExecutionStart(_) => "tool_execution_start",
            Self::ToolExecutionUpdate(_) => "tool_execution_update",
            Self::ToolExecutionEnd(_) => "tool_execution_end",
            Self::ModelSelect(_) => "model_select",
            Self::ThinkingLevelSelect(_) => "thinking_level_select",
            Self::UserBash(_) => "user_bash",
            Self::Input(_) => "input",
            Self::ToolCall(_) => "tool_call",
            Self::ToolResult(_) => "tool_result",
            Self::SessionBeforeCompact(_) => "session_before_compact",
            Self::SessionCompact(_) => "session_compact",
            Self::SessionBeforeTree(_) => "session_before_tree",
            Self::SessionTree(_) => "session_tree",
        }
    }
}

pub type EventResult = Result<Option<serde_json::Value>, String>;
pub type EventHandler = Arc<dyn Fn(&ExtensionEvent) -> EventResult + Send + Sync>;

// ============================================================================
// Tool Definition
// ============================================================================

/// Tool parameter schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub schema: serde_json::Value,
}

/// A tool registered by an extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub label: String,
    pub description: String,
    pub parameters: Vec<ToolParameter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_guidelines: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render_shell: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_mode: Option<String>,
}

/// Registered tool with source info
#[derive(Debug, Clone)]
pub struct RegisteredTool {
    pub definition: ToolDefinition,
    pub extension_path: String,
}

// ============================================================================
// Command Registration
// ============================================================================

#[derive(Debug, Clone)]
pub struct RegisteredCommand {
    pub name: String,
    pub description: Option<String>,
    pub extension_path: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedCommand {
    pub name: String,
    pub invocation_name: String,
    pub description: Option<String>,
    pub extension_path: String,
}

// ============================================================================
// Shortcuts & Flags
// ============================================================================

#[derive(Debug, Clone)]
pub struct ExtensionShortcut {
    pub shortcut: String,
    pub description: Option<String>,
    pub extension_path: String,
}

#[derive(Debug, Clone)]
pub struct ExtensionFlag {
    pub name: String,
    pub description: Option<String>,
    pub flag_type: FlagType,
    pub default: Option<FlagValue>,
    pub extension_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagType {
    Boolean,
    String,
}

#[derive(Debug, Clone)]
pub enum FlagValue {
    Bool(bool),
    Str(String),
}

// ============================================================================
// Extension struct
// ============================================================================

/// A loaded extension with all registered handlers and items
#[derive(Clone)]
pub struct Extension {
    pub path: String,
    pub resolved_path: String,
    pub handlers: HashMap<String, Vec<EventHandler>>,
    pub tools: HashMap<String, RegisteredTool>,
    pub commands: HashMap<String, RegisteredCommand>,
    pub flags: HashMap<String, ExtensionFlag>,
    pub shortcuts: HashMap<String, ExtensionShortcut>,
}

impl std::fmt::Debug for Extension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Extension")
            .field("path", &self.path)
            .field("handlers", &self.handlers.keys().collect::<Vec<_>>())
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .field("commands", &self.commands.keys().collect::<Vec<_>>())
            .field("flags", &self.flags.keys().collect::<Vec<_>>())
            .field("shortcuts", &self.shortcuts.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Extension {
    pub fn new(path: String, resolved_path: String) -> Self {
        Self { path, resolved_path, handlers: HashMap::new(), tools: HashMap::new(), commands: HashMap::new(), flags: HashMap::new(), shortcuts: HashMap::new() }
    }
}

// ============================================================================
// Event Results
// ============================================================================

#[derive(Debug, Clone)]
pub struct SessionBeforeSwitchResult {
    pub cancel: bool,
}

#[derive(Debug, Clone)]
pub struct SessionBeforeForkResult {
    pub cancel: bool,
    pub skip_conversation_restore: bool,
}

#[derive(Debug, Clone)]
pub struct BeforeAgentStartResult {
    pub system_prompt: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ToolCallEventResult {
    pub block: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UserBashEventResult {
    pub result: Option<serde_json::Value>,
}

// ============================================================================
// Load results
// ============================================================================

#[derive(Debug, Clone)]
pub struct LoadExtensionsResult {
    pub extensions: Vec<Extension>,
    pub errors: Vec<ExtensionLoadError>,
}

#[derive(Debug, Clone)]
pub struct ExtensionLoadError {
    pub path: String,
    pub error: String,
}

// ============================================================================
// Extension Factory
// ============================================================================

/// Extension factory trait - extensions implement this to register handlers
#[async_trait::async_trait]
pub trait ExtensionFactory: Send + Sync {
    fn name(&self) -> &str;
    async fn init(&self, api: &dyn ExtensionAPI) -> Result<(), String>;
}

/// ExtensionAPI trait - provided to extensions during initialization
pub trait ExtensionAPI: Send + Sync {
    /// Register an event handler
    fn on_raw(&self, event_type: &str, handler: EventHandler);

    /// Register a tool
    fn register_tool(&self, tool: ToolDefinition);
    fn register_command(&self, name: &str, description: Option<String>);
    fn register_shortcut(&self, shortcut: &str, description: Option<String>);
    fn register_flag(&self, name: &str, description: Option<String>, flag_type: FlagType, default: Option<FlagValue>);
}
