//! Agent state management

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use pick_ai::types::{ContentBlock, Message, Model};
use tokio::sync::mpsc;

use crate::agent_registry::AgentRegistry;
use crate::permission::fs_policy::FileSystemPolicy;
use crate::permission::manager::PermissionManager;
use crate::permission::sandbox::Sandbox as SandboxTrait;

/// Thinking/reasoning level for models
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThinkingLevel {
    Off,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}

impl From<pick_ai::types::ThinkingLevel> for ThinkingLevel {
    fn from(l: pick_ai::types::ThinkingLevel) -> Self {
        match l {
            pick_ai::types::ThinkingLevel::Off => ThinkingLevel::Off,
            pick_ai::types::ThinkingLevel::Minimal => ThinkingLevel::Minimal,
            pick_ai::types::ThinkingLevel::Low => ThinkingLevel::Low,
            pick_ai::types::ThinkingLevel::Medium => ThinkingLevel::Medium,
            pick_ai::types::ThinkingLevel::High => ThinkingLevel::High,
            pick_ai::types::ThinkingLevel::XHigh => ThinkingLevel::XHigh,
        }
    }
}

/// Tool execution mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolExecutionMode {
    Sequential,
    Parallel,
}

/// A tool available to the agent
#[derive(Clone)]
pub struct AgentTool {
    pub name: String,
    pub description: String,
    pub prompt_snippet: Option<String>,
    pub prompt_guidelines: Vec<String>,
    pub label: String,
    pub parameters: pick_ai::types::JsonSchema,
    pub execute: std::sync::Arc<
        dyn Send
            + Sync
            + Fn(
                String,
                serde_json::Value,
                ToolContext,
            )
                -> std::pin::Pin<Box<dyn Send + Future<Output = Result<AgentToolResult, String>>>>,
    >,
    pub execution_mode: ToolExecutionMode,
}

/// Async approval callback: given a title and message, returns true if approved
pub type ApproveFn = std::sync::Arc<
    dyn Send
        + Sync
        + Fn(String, String) -> std::pin::Pin<Box<dyn Send + std::future::Future<Output = bool>>>,
>;

/// A question option presented to the user
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}

/// A question prompt presented to the user
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuestionPrompt {
    pub question: String,
    pub header: String,
    pub options: Vec<QuestionOption>,
    #[serde(default)]
    pub multiple: bool,
}

/// Async question callback: takes questions, returns answers (one Vec<String> per question)
pub type QuestionFn = std::sync::Arc<
    dyn Send
        + Sync
        + Fn(
            Vec<QuestionPrompt>,
        ) -> std::pin::Pin<
            Box<dyn Send + std::future::Future<Output = Result<Vec<Vec<String>>, String>>>,
        >,
>;

/// Context passed to tool execute functions, including cancellation and progress reporting.
#[derive(Default)]
pub struct ToolContext {
    pub cancel: Option<tokio::sync::watch::Receiver<bool>>,
    pub progress: Option<mpsc::UnboundedSender<String>>,
    pub approve: Option<ApproveFn>,
    pub question: Option<QuestionFn>,
    pub agent_id: Option<String>,
    pub agent_registry: Option<Arc<AgentRegistry>>,
    pub default_model: Option<Model>,
    pub fs_policy: Option<Arc<FileSystemPolicy>>,
    pub cwd: Option<std::path::PathBuf>,
    pub permission_manager: Option<Arc<PermissionManager>>,
    pub sandbox: Option<Arc<dyn SandboxTrait>>,
    pub sandbox_enabled: Option<Arc<AtomicBool>>,
    pub tool_event_bus: Option<std::sync::Arc<super::hooks::ToolEventBus>>,
}

impl Clone for ToolContext {
    fn clone(&self) -> Self {
        Self {
            cancel: self.cancel.clone(),
            progress: self.progress.clone(),
            approve: self.approve.clone(),
            question: self.question.clone(),
            agent_id: self.agent_id.clone(),
            agent_registry: self.agent_registry.clone(),
            default_model: self.default_model.clone(),
            fs_policy: self.fs_policy.clone(),
            cwd: self.cwd.clone(),
            permission_manager: self.permission_manager.clone(),
            sandbox: self.sandbox.clone(),
            sandbox_enabled: self.sandbox_enabled.clone(),
            tool_event_bus: self.tool_event_bus.clone(),
        }
    }
}

impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext")
            .field("cancel", &self.cancel.as_ref().map(|_| "Receiver"))
            .field("progress", &self.progress.as_ref().map(|_| "Sender"))
            .field("approve", &self.approve.as_ref().map(|_| "ApproveFn"))
            .field("question", &self.question.as_ref().map(|_| "QuestionFn"))
            .field("agent_id", &self.agent_id)
            .field(
                "agent_registry",
                &self.agent_registry.as_ref().map(|_| "AgentRegistry"),
            )
            .field("default_model", &self.default_model.as_ref().map(|m| &m.id))
            .field(
                "sandbox_enabled",
                &self
                    .sandbox_enabled
                    .as_ref()
                    .map(|a| a.load(Ordering::Relaxed)),
            )
            .field(
                "tool_event_bus",
                &self.tool_event_bus.as_ref().map(|_| "ToolEventBus"),
            )
            .finish()
    }
}

/// Result from a tool execution
#[derive(Debug, Clone)]
pub struct AgentToolResult {
    pub content: Vec<ContentBlock>,
    pub is_error: bool,
    pub terminate: bool,
}

/// Agent state
pub struct AgentState {
    pub system_prompt: String,
    pub model: Model,
    pub thinking_level: ThinkingLevel,
    pub tools: Vec<AgentTool>,
    pub messages: Vec<Message>,
    pub is_streaming: bool,
    pub pending_tool_calls: Vec<String>,
    pub error_message: Option<String>,
    pub consecutive_tool_errors: u32,
    pub plan_awareness_triggered: bool,
}

/// Agent context snapshot
#[derive(Clone)]
pub struct AgentContext {
    pub system_prompt: String,
    pub messages: Vec<Message>,
    pub tools: Vec<AgentTool>,
}
