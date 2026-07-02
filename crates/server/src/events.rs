use pick_agent::core::events::AgentEvent;
use pick_ai::types::{ContentBlock, Message};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct WsEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageUpdatePayload {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    pub delta: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThinkingPayload {
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolStartPayload {
    pub tool_call_id: String,
    pub tool_name: String,
    pub args: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolUpdatePayload {
    pub tool_call_id: String,
    pub partial_output: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolEndPayload {
    pub tool_call_id: String,
    pub tool_name: String,
    pub output: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsagePayload {
    pub input: u64,
    pub output: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentEndPayload {
    pub usage: UsagePayload,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApprovalRequiredPayload {
    pub id: String,
    pub tool_name: String,
    pub tool_args: String,
    pub permission: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuestionPayload {
    pub id: String,
    pub prompts: Vec<QuestionPromptPayload>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuestionPromptPayload {
    pub question: String,
    pub value_hint: Option<String>,
    pub r#type: String,
}

pub fn serialize_event(event: &AgentEvent) -> Vec<WsEvent> {
    match event {
        AgentEvent::MessageUpdate { message, .. } => {
            if let Message::Assistant(msg) = message {
                let mut text = String::new();
                let mut thinking: Option<String> = None;

                for block in &msg.content {
                    match block {
                        ContentBlock::Text(t) => {
                            text.push_str(&t.text);
                        }
                        ContentBlock::Thinking(t) if !t.thinking.is_empty() => {
                            thinking = Some(t.thinking.clone());
                        }
                        _ => {}
                    }
                }

                if !text.is_empty() || thinking.is_some() {
                    vec![WsEvent {
                        event_type: "message_update".to_string(),
                        payload: serde_json::to_value(MessageUpdatePayload {
                            text,
                            thinking,
                            delta: false,
                        })
                        .unwrap_or_default(),
                    }]
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        }
        AgentEvent::ToolExecutionStart {
            tool_name,
            tool_call_id,
            args,
            ..
        } => vec![WsEvent {
            event_type: "tool_start".to_string(),
            payload: serde_json::to_value(ToolStartPayload {
                tool_call_id: tool_call_id.clone(),
                tool_name: tool_name.clone(),
                args: args.clone(),
            })
            .unwrap_or_default(),
        }],
        AgentEvent::ToolExecutionUpdate {
            tool_call_id,
            partial_result,
            ..
        } => {
            let partial_output = partial_result
                .get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .unwrap_or("");
            vec![WsEvent {
                event_type: "tool_update".to_string(),
                payload: serde_json::to_value(ToolUpdatePayload {
                    tool_call_id: tool_call_id.clone(),
                    partial_output: partial_output.to_string(),
                })
                .unwrap_or_default(),
            }]
        }
        AgentEvent::ToolExecutionEnd {
            tool_call_id,
            tool_name,
            result,
            is_error,
            ..
        } => {
            let output = if *is_error {
                result
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error")
                    .to_string()
            } else if let Some(texts) = result.get("content").and_then(|c| c.as_array()) {
                texts
                    .iter()
                    .filter_map(|t| t.as_str())
                    .collect::<Vec<_>>()
                    .join("")
            } else {
                String::new()
            };
            vec![WsEvent {
                event_type: "tool_end".to_string(),
                payload: serde_json::to_value(ToolEndPayload {
                    tool_call_id: tool_call_id.clone(),
                    tool_name: tool_name.clone(),
                    output,
                    is_error: *is_error,
                })
                .unwrap_or_default(),
            }]
        }
        AgentEvent::TurnEnd { .. } => vec![WsEvent {
            event_type: "turn_end".to_string(),
            payload: Value::Null,
        }],
        _ => Vec::new(),
    }
}

pub fn serialize_agent_end(usage_input: u64, usage_output: u64, title: Option<String>) -> WsEvent {
    WsEvent {
        event_type: "agent_end".to_string(),
        payload: serde_json::to_value(AgentEndPayload {
            usage: UsagePayload {
                input: usage_input,
                output: usage_output,
            },
            title,
        })
        .unwrap_or_default(),
    }
}
