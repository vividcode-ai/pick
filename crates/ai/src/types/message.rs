//! Message types for AI conversations.

use serde::{Deserialize, Serialize};

use super::content::ContentBlock;
use super::stream::StopReason;

/// A user message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    pub role: UserMessageRole,
    pub content: Vec<ContentBlock>,
    /// Unix timestamp in milliseconds
    pub timestamp: i64,
}

impl Default for UserMessage {
    fn default() -> Self {
        Self {
            role: UserMessageRole::User,
            content: Vec::new(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UserMessageRole {
    #[serde(rename = "user")]
    User,
}

impl UserMessage {
    pub fn new(content: impl Into<Vec<ContentBlock>>) -> Self {
        Self {
            role: UserMessageRole::User,
            content: content.into(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self::new(vec![ContentBlock::text(text)])
    }
}

/// A diagnostic entry attached to an assistant message (e.g. tool errors, warnings).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessageDiagnostic {
    #[serde(rename = "type")]
    pub diag_type: String,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<DiagnosticError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticError {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// An assistant message (model response)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub role: AssistantMessageRole,
    pub content: Vec<ContentBlock>,
    pub api: String,
    pub provider: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_id: Option<String>,
    pub usage: super::model::Usage,
    pub stop_reason: StopReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Vec<AssistantMessageDiagnostic>>,
    /// Unix timestamp in milliseconds
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AssistantMessageRole {
    #[serde(rename = "assistant")]
    Assistant,
}

impl AssistantMessage {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        content: Vec<ContentBlock>,
        api: String,
        provider: String,
        model: String,
        usage: super::model::Usage,
        stop_reason: StopReason,
    ) -> Self {
        Self {
            role: AssistantMessageRole::Assistant,
            content,
            api,
            provider,
            model,
            response_model: None,
            response_id: None,
            usage,
            stop_reason,
            error_message: None,
            diagnostics: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// A tool result message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultMessage {
    pub role: ToolResultMessageRole,
    pub tool_call_id: String,
    pub tool_name: String,
    pub content: Vec<ContentBlock>,
    pub is_error: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    /// Unix timestamp in milliseconds
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ToolResultMessageRole {
    #[serde(rename = "toolResult")]
    ToolResult,
}

impl ToolResultMessage {
    pub fn new(
        tool_call_id: impl Into<String>,
        tool_name: impl Into<String>,
        content: Vec<ContentBlock>,
        is_error: bool,
    ) -> Self {
        Self {
            role: ToolResultMessageRole::ToolResult,
            tool_call_id: tool_call_id.into(),
            tool_name: tool_name.into(),
            content,
            is_error,
            details: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// A message in the conversation (enum over all message types)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role")]
pub enum Message {
    #[serde(rename = "user")]
    User(UserMessage),
    #[serde(rename = "assistant")]
    Assistant(AssistantMessage),
    #[serde(rename = "toolResult")]
    ToolResult(ToolResultMessage),
}

impl Message {
    pub fn timestamp(&self) -> i64 {
        match self {
            Message::User(m) => m.timestamp,
            Message::Assistant(m) => m.timestamp,
            Message::ToolResult(m) => m.timestamp,
        }
    }
}

impl From<UserMessage> for Message {
    fn from(m: UserMessage) -> Self {
        Message::User(m)
    }
}

impl From<AssistantMessage> for Message {
    fn from(m: AssistantMessage) -> Self {
        Message::Assistant(m)
    }
}

impl From<ToolResultMessage> for Message {
    fn from(m: ToolResultMessage) -> Self {
        Message::ToolResult(m)
    }
}

/// Context sent to the AI provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
}

use super::tool::ToolDefinition;
