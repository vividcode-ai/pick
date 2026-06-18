//! Stream types for AI response streaming.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::content::{ContentBlock, ToolCall};
use super::model::Usage;

/// Why a response stopped
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StopReason {
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "length")]
    Length,
    #[serde(rename = "toolUse")]
    ToolUse,
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "aborted")]
    Aborted,
}

/// Stream options common to all providers
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreamOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_retention: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retry_delay_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
    /// Signal receiver for cancellation
    #[serde(skip)]
    pub signal: Option<std::sync::Arc<tokio::sync::watch::Receiver<bool>>>,
}

/// Simple stream options with reasoning support
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SimpleStreamOptions {
    #[serde(flatten)]
    pub base: StreamOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}

/// Events emitted during streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamEvent {
    Start {
        partial: PartialAssistantMessage,
    },
    TextStart {
        content_index: usize,
        partial: PartialAssistantMessage,
    },
    TextDelta {
        content_index: usize,
        delta: String,
        partial: PartialAssistantMessage,
    },
    TextEnd {
        content_index: usize,
        content: String,
        partial: PartialAssistantMessage,
    },
    ThinkingStart {
        content_index: usize,
        partial: PartialAssistantMessage,
    },
    ThinkingDelta {
        content_index: usize,
        delta: String,
        partial: PartialAssistantMessage,
    },
    ThinkingEnd {
        content_index: usize,
        content: String,
        partial: PartialAssistantMessage,
    },
    ToolCallStart {
        content_index: usize,
        partial: PartialAssistantMessage,
    },
    ToolCallDelta {
        content_index: usize,
        delta: String,
        partial: PartialAssistantMessage,
    },
    ToolCallEnd {
        content_index: usize,
        tool_call: ToolCall,
        partial: PartialAssistantMessage,
    },
    Done {
        reason: StopReason,
        message: super::message::AssistantMessage,
    },
    Error {
        reason: StopReason,
        error: super::message::AssistantMessage,
    },
}

/// Partial assistant message during streaming
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PartialAssistantMessage {
    pub content: Vec<ContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub timestamp: i64,
}

/// A stream of assistant message events
pub struct AssistantMessageEventStream {
    receiver: tokio::sync::mpsc::Receiver<StreamEvent>,
}

impl AssistantMessageEventStream {
    pub fn new(receiver: tokio::sync::mpsc::Receiver<StreamEvent>) -> Self {
        Self { receiver }
    }

    /// Collect the next event from the stream
    pub async fn next_event(&mut self) -> Option<StreamEvent> {
        self.receiver.recv().await
    }

    /// Collect all events into a final AssistantMessage
    pub async fn collect(&mut self) -> super::message::AssistantMessage {
        let mut last_message = super::message::AssistantMessage::new(
            Vec::new(),
            String::new(),
            String::new(),
            String::new(),
            Usage::zero(),
            StopReason::Stop,
        );

        while let Some(event) = self.next_event().await {
            match event {
                StreamEvent::Done { message, .. } => {
                    last_message = message;
                    break;
                }
                StreamEvent::Error { error, .. } => {
                    last_message = error;
                    break;
                }
                _ => {}
            }
        }

        last_message
    }
}
