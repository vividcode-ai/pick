//! Session entry types

use serde::{Deserialize, Serialize};

use pick_ai::types::Message;
use pick_ai::types::content::ContentBlock;
use pick_ai::types::message::{AssistantMessage, ToolResultMessage, UserMessage};
use pick_ai::types::model::Usage;
use pick_ai::types::stream::StopReason;

/// A single entry in a session.
/// Entries can form a tree via the optional parent_id field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub id: String,
    /// ID of the parent entry (None for root entries).
    /// Enables tree navigation, branching, and fork history.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub timestamp: i64,
    #[serde(flatten)]
    pub kind: SessionEntryKind,
}

/// Types of session entries
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SessionEntryKind {
    #[serde(rename = "message")]
    Message(MessageEntry),
    #[serde(rename = "compaction")]
    Compaction(CompactionEntry),
    #[serde(rename = "branch_summary")]
    BranchSummary(BranchSummaryEntry),
    #[serde(rename = "model_change")]
    ModelChange(ModelChangeEntry),
    #[serde(rename = "thinking_level_change")]
    ThinkingLevelChange(ThinkingLevelChangeEntry),
    #[serde(rename = "custom")]
    Custom(CustomEntry),
    #[serde(rename = "session_info")]
    SessionInfo(SessionInfoEntry),
    #[serde(rename = "leaf_change")]
    LeafChange(LeafChangeEntry),
    #[serde(rename = "label")]
    Label(LabelEntry),
    #[serde(rename = "agent_mode_change")]
    AgentModeChange(AgentModeChangeEntry),
    #[serde(rename = "todo_update")]
    TodoUpdate(TodoUpdateEntry),
    #[serde(rename = "goal")]
    Goal(GoalEntry),
}

/// A message entry in the session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEntry {
    pub role: String,
    pub content: serde_json::Value,
    pub api: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub usage: Option<serde_json::Value>,
    pub stop_reason: Option<String>,
}

/// A compaction entry (context window management)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionEntry {
    pub summary: String,
    pub token_count: Option<u64>,
}

/// A branch summary entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchSummaryEntry {
    pub summary: String,
}

/// A model change entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelChangeEntry {
    pub from: String,
    pub to: String,
}

/// A thinking level change entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingLevelChangeEntry {
    pub from: String,
    pub to: String,
}

/// A custom entry type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomEntry {
    pub kind: String,
    pub data: serde_json::Value,
}

/// Session info entry (name setting)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfoEntry {
    pub name: String,
}

/// Leaf change entry (branch navigation record)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeafChangeEntry {
    pub from: Option<String>,
    pub to: String,
}

/// Agent mode change entry (build <-> plan switch)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentModeChangeEntry {
    pub from: String,
    pub to: String,
}

/// Label entry (user-assigned label for another entry)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelEntry {
    pub target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// A single todo item in a task list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub content: String,
    pub status: String,
    pub priority: String,
}

/// Todo update entry — stores a snapshot of the full todo list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoUpdateEntry {
    pub todos: Vec<TodoItem>,
}

/// Goal state entry — stores the current thread goal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalEntry {
    pub objective: String,
    /// Verifiable completion criterion; empty string means not set
    #[serde(default)]
    pub completion_criterion: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<i64>,
    pub tokens_used: i64,
    pub time_used_seconds: i64,
    /// Maximum number of automatic continuation turns before the goal enters `usage_limited` status.
    /// `None` means unlimited continuations.
    #[serde(default)]
    pub max_turns: Option<u32>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Session header metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHeader {
    pub id: String,
    pub version: u64,
    pub created_at: i64,
    pub updated_at: i64,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub thinking_level: Option<String>,
    #[serde(default)]
    pub archived: bool,
}

impl From<&Message> for SessionEntry {
    fn from(msg: &Message) -> Self {
        let (role, content, api, provider, model, usage, stop_reason) = match msg {
            Message::User(u) => (
                "user",
                serde_json::to_value(&u.content).unwrap_or_default(),
                None,
                None,
                None,
                None,
                None,
            ),
            Message::Assistant(a) => (
                "assistant",
                serde_json::to_value(&a.content).unwrap_or_default(),
                Some(a.api.clone()),
                Some(a.provider.clone()),
                Some(a.model.clone()),
                Some(serde_json::to_value(&a.usage).unwrap_or_default()),
                Some(format!("{:?}", a.stop_reason)),
            ),
            Message::ToolResult(t) => (
                "tool_result",
                serde_json::to_value(&t.content).unwrap_or_default(),
                None,
                None,
                None,
                None,
                None,
            ),
        };

        SessionEntry {
            id: uuid::Uuid::now_v7().to_string(),
            parent_id: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
            kind: SessionEntryKind::Message(MessageEntry {
                role: role.to_string(),
                content,
                api,
                provider,
                model,
                usage,
                stop_reason,
            }),
        }
    }
}

/// Convert a `ContentBlock` vec stored as `serde_json::Value` back into `Vec<ContentBlock>`.
fn content_value_to_blocks(value: &serde_json::Value) -> Vec<ContentBlock> {
    match value {
        serde_json::Value::String(s) => vec![ContentBlock::text(s)],
        serde_json::Value::Array(arr) => {
            // Try to deserialize each element as a ContentBlock
            let mut blocks = Vec::new();
            for item in arr {
                if let Ok(block) = serde_json::from_value::<ContentBlock>(item.clone()) {
                    blocks.push(block);
                }
            }
            blocks
        }
        _ => Vec::new(),
    }
}

/// Fallible conversion from a `SessionEntry` to a `Message`.
/// Returns `None` for non-message entry kinds.
impl TryFrom<&SessionEntry> for Message {
    type Error = ();

    fn try_from(entry: &SessionEntry) -> Result<Self, Self::Error> {
        match &entry.kind {
            SessionEntryKind::Message(msg) => {
                match msg.role.as_str() {
                    "user" => Ok(Message::User(UserMessage {
                        role: pick_ai::types::message::UserMessageRole::User,
                        content: content_value_to_blocks(&msg.content),
                        timestamp: entry.timestamp,
                    })),
                    "assistant" => {
                        let usage = msg
                            .usage
                            .as_ref()
                            .and_then(|u| serde_json::from_value::<Usage>(u.clone()).ok())
                            .unwrap_or_else(Usage::zero);
                        let stop_reason = msg
                            .stop_reason
                            .as_deref()
                            .and_then(|s| {
                                serde_json::from_str::<StopReason>(&format!("\"{}\"", s)).ok()
                            })
                            .unwrap_or(StopReason::Stop);
                        Ok(Message::Assistant(AssistantMessage {
                            role: pick_ai::types::message::AssistantMessageRole::Assistant,
                            content: content_value_to_blocks(&msg.content),
                            api: msg.api.clone().unwrap_or_default(),
                            provider: msg.provider.clone().unwrap_or_default(),
                            model: msg.model.clone().unwrap_or_default(),
                            response_model: None,
                            response_id: None,
                            usage,
                            stop_reason,
                            error_message: None,
                            diagnostics: None,
                            timestamp: entry.timestamp,
                        }))
                    }
                    "tool_result" => {
                        // Extract tool_call_id and tool_name from content blocks if available
                        let tool_call_id = String::new();
                        let tool_name = String::new();
                        let content = content_value_to_blocks(&msg.content);
                        Ok(Message::ToolResult(ToolResultMessage {
                            role: pick_ai::types::message::ToolResultMessageRole::ToolResult,
                            tool_call_id,
                            tool_name,
                            content,
                            is_error: false,
                            details: None,
                            timestamp: entry.timestamp,
                        }))
                    }
                    _ => Err(()),
                }
            }
            _ => Err(()),
        }
    }
}
