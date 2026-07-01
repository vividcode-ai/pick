//! TodoPlan tool - manages task list

use pick_ai::types::ContentBlock;

use crate::core::state::{AgentTool, AgentToolResult, ToolContext, ToolExecutionMode};
use crate::session::entries::TodoItem;

/// Create the todo_plan tool
pub fn create_todo_plan_tool() -> AgentTool {
    let params = pick_ai::types::JsonSchema {
        schema_type: "object".to_string(),
        properties: Some(
            vec![(
                "todos".to_string(),
                serde_json::json!({
                    "type": "array",
                    "description": "The updated todo list",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": { "type": "string", "description": "Brief description of the task" },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed", "cancelled"],
                                "description": "Current status of the task"
                            },
                            "priority": {
                                "type": "string",
                                "enum": ["high", "medium", "low"],
                                "description": "Priority level of the task"
                            }
                        },
                        "required": ["content", "status", "priority"]
                    }
                }),
            )]
            .into_iter()
            .collect(),
        ),
        required: Some(vec!["todos".to_string()]),
        description: Some("Create and manage a structured todo/task list. Creates, updates, or replaces the task list for the current session.".to_string()),
        items: None,
        additional_properties: Some(false),
    };

    AgentTool {
        name: "todo_plan".to_string(),
        description: "Create and manage a structured task list. Use this to track progress on complex multi-step tasks.".to_string(),
        prompt_snippet: Some("Manage task list with todo_plan tool".to_string()),
        prompt_guidelines: vec![],
        label: "todo_plan".to_string(),
        parameters: params,
        execute: std::sync::Arc::new(move |_tool_call_id, args, ctx: ToolContext| {
            Box::pin(async move {
                let todos_val = args.get("todos")
                    .ok_or_else(|| "Missing 'todos' argument".to_string())?;

                let todos: Vec<TodoItem> = serde_json::from_value(todos_val.clone())
                    .map_err(|e| format!("Invalid todos format: {}", e))?;

                let title = format!("{} todos", todos.iter().filter(|t| t.status != "completed").count());

                // Emit event for TUI rendering
                if let Some(handler) = ctx.progress.as_ref() {
                    let _ = handler.send(serde_json::json!({"todos": &todos}).to_string());
                }

                // Permission check
                if let Some(ref approve) = ctx.approve
                    && !approve("todo_plan".to_string(), title.clone()).await {
                        return Ok(AgentToolResult {
                            content: vec![ContentBlock::text("Permission denied for todo_plan")],
                            is_error: true,
                            terminate: false,
                        });
                    }

                Ok(AgentToolResult {
                    content: vec![ContentBlock::text(serde_json::to_string_pretty(&todos).unwrap_or_default())],
                    is_error: false,
                    terminate: false,
                })
            })
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}
