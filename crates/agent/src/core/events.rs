//! Agent event types for UI updates

use pick_ai::types::{Message, ToolResultMessage};

/// Events emitted by the agent during execution
#[derive(Debug, Clone)]
pub enum AgentEvent {
    // Agent lifecycle
    AgentStart,
    AgentEnd { messages: Vec<Message> },
    // Turn lifecycle
    TurnStart,
    TurnEnd {
        message: Message,
        tool_results: Vec<ToolResultMessage>,
    },
    // Message lifecycle
    MessageStart { message: Message },
    MessageUpdate { message: Message, assistant_message_event: Option<serde_json::Value> },
    MessageEnd { message: Message },
    // Tool execution lifecycle
    ToolExecutionStart {
        tool_call_id: String,
        tool_name: String,
        args: serde_json::Value,
    },
    ToolExecutionUpdate {
        tool_call_id: String,
        tool_name: String,
        args: serde_json::Value,
        partial_result: serde_json::Value,
    },
    ToolExecutionEnd {
        tool_call_id: String,
        tool_name: String,
        result: serde_json::Value,
        is_error: bool,
    },
    // Auto-retry lifecycle
    AutoRetryStart {
        attempt: u32,
        max_attempts: u32,
        delay_ms: u64,
        error_message: String,
    },
    AutoRetryEnd {
        success: bool,
        attempt: u32,
        final_error: Option<String>,
    },
    // Tool state updates for TUI rendering
    TodoUpdated {
        todos: serde_json::Value,
    },
    GoalUpdated {
        goal: serde_json::Value,
    },
}

/// Callback type for agent events
pub type AgentEventHandler = std::sync::Arc<dyn Send + Sync + Fn(AgentEvent)>;

/// Convert an AgentEvent to a JSON Value for stdout streaming
pub fn agent_event_to_json_value(event: &AgentEvent) -> serde_json::Value {
    use AgentEvent::*;
    match event {
        AgentStart => serde_json::json!({"type": "agent_start"}),
        AgentEnd { messages } => serde_json::json!({
            "type": "agent_end",
            "messages": messages,
        }),
        TurnStart => serde_json::json!({"type": "turn_start"}),
        TurnEnd { message, tool_results } => serde_json::json!({
            "type": "turn_end",
            "message": message,
            "tool_results": tool_results,
        }),
        MessageStart { message } => serde_json::json!({
            "type": "message_start",
            "message": message,
        }),
        MessageUpdate { message, assistant_message_event } => serde_json::json!({
            "type": "message_update",
            "message": message,
            "assistant_message_event": assistant_message_event,
        }),
        MessageEnd { message } => serde_json::json!({
            "type": "message_end",
            "message": message,
        }),
        ToolExecutionStart { tool_call_id, tool_name, args } => serde_json::json!({
            "type": "tool_execution_start",
            "tool_call_id": tool_call_id,
            "tool_name": tool_name,
            "args": args,
        }),
        ToolExecutionUpdate { tool_call_id, tool_name, args, partial_result } => serde_json::json!({
            "type": "tool_execution_update",
            "tool_call_id": tool_call_id,
            "tool_name": tool_name,
            "args": args,
            "partial_result": partial_result,
        }),
        ToolExecutionEnd { tool_call_id, tool_name, result, is_error } => serde_json::json!({
            "type": "tool_execution_end",
            "tool_call_id": tool_call_id,
            "tool_name": tool_name,
            "result": result,
            "is_error": is_error,
        }),
        AutoRetryStart { attempt, max_attempts, delay_ms, error_message } => serde_json::json!({
            "type": "auto_retry_start",
            "attempt": attempt,
            "max_attempts": max_attempts,
            "delay_ms": delay_ms,
            "error_message": error_message,
        }),
        AutoRetryEnd { success, attempt, final_error } => serde_json::json!({
            "type": "auto_retry_end",
            "success": success,
            "attempt": attempt,
            "final_error": final_error,
        }),
        TodoUpdated { todos } => serde_json::json!({
            "type": "todo_updated",
            "todos": todos,
        }),
        GoalUpdated { goal } => serde_json::json!({
            "type": "goal_updated",
            "goal": goal,
        }),
    }
}
