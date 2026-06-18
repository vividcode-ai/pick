//! Message types used across the agent session


use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A custom message embedded in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomMessage<T = Value> {
    pub custom_type: String,
    pub content: T,
    pub display: bool,
    pub details: Option<Value>,
}

/// A bash execution message embedded in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashExecutionMessage {
    pub command: String,
    pub output: String,
    pub exit_code: Option<i32>,
    pub cancelled: bool,
    pub truncated: bool,
    pub full_output_path: Option<String>,
    pub timestamp: i64,
}

/// Create a custom message value for conversation entries
pub fn create_custom_message(
    custom_type: String,
    content: Value,
    display: bool,
    details: Option<Value>,
    timestamp: i64,
) -> Value {
    serde_json::json!({
        "role": "custom",
        "customType": custom_type,
        "content": content,
        "display": display,
        "details": details,
        "timestamp": timestamp,
    })
}

/// Create a branch summary message value
pub fn create_branch_summary_message(summary: String, from_id: String, timestamp: i64) -> Value {
    serde_json::json!({
        "role": "branchSummary",
        "summary": summary,
        "fromId": from_id,
        "timestamp": timestamp,
    })
}

/// Create a compaction summary message value
pub fn create_compaction_summary_message(summary: String, tokens_before: usize, timestamp: i64) -> Value {
    serde_json::json!({
        "role": "compactionSummary",
        "summary": summary,
        "tokensBefore": tokens_before,
        "timestamp": timestamp,
    })
}

/// Convert agent messages to LLM-compatible format
pub fn convert_to_llm_messages(messages: &[Value]) -> Vec<Value> {
    messages.iter().map(|msg| {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
        match role {
            "user" | "assistant" | "toolResult" => msg.clone(),
            "custom" => {
                let custom_type = msg.get("customType").and_then(|v| v.as_str()).unwrap_or("");
                let content = msg.get("content").cloned().unwrap_or(Value::Null);
                serde_json::json!({
                    "role": "custom",
                    "customType": custom_type,
                    "content": content,
                })
            }
            _ => msg.clone(),
        }
    }).collect()
}
