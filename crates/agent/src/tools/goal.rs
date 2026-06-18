use std::sync::Arc;

use pick_ai::types::ContentBlock;

use crate::core::state::{AgentTool, AgentToolResult, ToolContext, ToolExecutionMode};
use crate::session::goal::GoalManager;

// ── Stub versions (backward compat, no real GoalManager) ──────────

pub fn create_get_goal_tool_with_mode() -> AgentTool {
    AgentTool {
        name: "get_goal".to_string(),
        description: "Get the current goal for this thread, including status and token usage.".to_string(),
        prompt_snippet: Some("Get current thread goal".to_string()),
        prompt_guidelines: vec![],
        label: "get_goal".to_string(),
        parameters: make_params(
            "Get the current goal for this thread, including status, budget, and token usage.",
            vec![],
            vec![],
        ),
        execute: std::sync::Arc::new(move |_tool_call_id, _args, _ctx: ToolContext| {
            Box::pin(async move {
                Ok(AgentToolResult {
                    content: vec![ContentBlock::text(
                        "{\"goal\": null, \"message\": \"No active goal for this thread.\"}"
                    )],
                    is_error: false,
                    terminate: false,
                })
            })
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}

pub fn create_create_goal_tool_with_mode() -> AgentTool {
    AgentTool {
        name: "create_goal".to_string(),
        description: "Create a goal only when explicitly requested by the user or system/developer instructions; \
                      do not infer goals from ordinary tasks."
            .to_string(),
        prompt_snippet: Some("Create a new thread goal".to_string()),
        prompt_guidelines: vec![],
        label: "create_goal".to_string(),
        parameters: make_params(
            "Create a new active goal. Fails if a goal already exists.",
            vec![
                (
                    "objective",
                    serde_json::json!({
                        "type": "string",
                        "description": "The concrete objective to start pursuing."
                    }),
                ),
                (
                    "token_budget",
                    serde_json::json!({
                        "type": "integer",
                        "description": "Optional positive token budget for the new active goal."
                    }),
                ),
            ],
            vec!["objective"],
        ),
        execute: std::sync::Arc::new(move |_tool_call_id, args, _ctx: ToolContext| {
            Box::pin(async move {
                let objective = args.get("objective")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing 'objective' argument".to_string())?;

                let token_budget = args.get("token_budget").and_then(|v| v.as_i64());

                let result = serde_json::json!({
                    "goal": {
                        "objective": objective,
                        "status": "active",
                        "tokenBudget": token_budget,
                        "tokensUsed": 0,
                        "timeUsedSeconds": 0,
                    },
                    "message": "Goal created. Use get_goal to check current status."
                });

                Ok(AgentToolResult {
                    content: vec![ContentBlock::text(serde_json::to_string_pretty(&result).unwrap_or_default())],
                    is_error: false,
                    terminate: false,
                })
            })
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}

pub fn create_update_goal_tool_with_mode() -> AgentTool {
    AgentTool {
        name: "update_goal".to_string(),
        description: "Update the existing goal. Use this tool only to mark the goal achieved or genuinely blocked."
            .to_string(),
        prompt_snippet: Some("Update current thread goal".to_string()),
        prompt_guidelines: vec![],
        label: "update_goal".to_string(),
        parameters: make_params(
            "Update the existing goal. Only marks complete or blocked.",
            vec![(
                "status",
                serde_json::json!({
                    "type": "string",
                    "enum": ["complete", "blocked"],
                    "description": "Set to `complete` only when the objective is achieved. \
                     Set to `blocked` only when genuinely stuck."
                }),
            )],
            vec!["status"],
        ),
        execute: std::sync::Arc::new(move |_tool_call_id, args, _ctx: ToolContext| {
            Box::pin(async move {
                let status = args.get("status")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing 'status' argument".to_string())?;

                if status != "complete" && status != "blocked" {
                    return Err("update_goal can only mark the existing goal complete or blocked".to_string());
                }

                let result = serde_json::json!({
                    "goal": {
                        "status": status,
                        "objective": null,
                        "tokensUsed": 0,
                    },
                    "message": format!("Goal marked as {}.", status)
                });

                Ok(AgentToolResult {
                    content: vec![ContentBlock::text(serde_json::to_string_pretty(&result).unwrap_or_default())],
                    is_error: false,
                    terminate: false,
                })
            })
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}

// ── Real versions (wired to GoalManager) ──────────────────────────

fn make_params(desc: &str, props: Vec<(&str, serde_json::Value)>, required: Vec<&str>) -> pick_ai::types::JsonSchema {
    pick_ai::types::JsonSchema {
        schema_type: "object".to_string(),
        properties: Some(props.into_iter().map(|(k, v)| (k.to_string(), v)).collect()),
        required: Some(required.into_iter().map(|s| s.to_string()).collect()),
        description: Some(desc.to_string()),
        items: None,
        additional_properties: Some(false),
    }
}

fn goal_entry_to_json(goal: &crate::session::entries::GoalEntry) -> serde_json::Value {
    serde_json::json!({
        "objective": goal.objective,
        "status": goal.status,
        "tokenBudget": goal.token_budget,
        "tokensUsed": goal.tokens_used,
        "timeUsedSeconds": goal.time_used_seconds,
        "createdAt": goal.created_at,
        "updatedAt": goal.updated_at,
    })
}

pub fn create_get_goal_tool(goal_manager: Arc<GoalManager>) -> AgentTool {
    AgentTool {
        name: "get_goal".to_string(),
        description: "Get the current goal for this thread, including status and token usage.".to_string(),
        prompt_snippet: Some("Get current thread goal".to_string()),
        prompt_guidelines: vec![],
        label: "get_goal".to_string(),
        parameters: make_params(
            "Get the current goal for this thread, including status, budget, and token usage.",
            vec![],
            vec![],
        ),
        execute: Arc::new(move |_tool_call_id, _args, _ctx: ToolContext| {
            let gm = goal_manager.clone();
            Box::pin(async move {
                match gm.get() {
                    Some(goal) => {
                        let remaining = gm.remaining_tokens();
                        let mut resp = serde_json::json!({
                            "goal": goal_entry_to_json(&goal),
                        });
                        if let Some(r) = remaining {
                            resp["remainingTokens"] = serde_json::json!(r);
                        }
                        Ok(AgentToolResult {
                            content: vec![ContentBlock::text(
                                serde_json::to_string_pretty(&resp).unwrap_or_default()
                            )],
                            is_error: false,
                            terminate: false,
                        })
                    }
                    None => Ok(AgentToolResult {
                        content: vec![ContentBlock::text(
                            "{\"goal\": null, \"message\": \"No active goal for this thread.\"}"
                        )],
                        is_error: false,
                        terminate: false,
                    }),
                }
            })
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}

pub fn create_create_goal_tool(goal_manager: Arc<GoalManager>) -> AgentTool {
    AgentTool {
        name: "create_goal".to_string(),
        description: "Create a goal only when explicitly requested by the user or system/developer instructions; \
                      do not infer goals from ordinary tasks."
            .to_string(),
        prompt_snippet: Some("Create a new thread goal".to_string()),
        prompt_guidelines: vec![],
        label: "create_goal".to_string(),
        parameters: make_params(
            "Create a new active goal. Fails if a goal already exists.",
            vec![
                (
                    "objective",
                    serde_json::json!({
                        "type": "string",
                        "description": "The concrete objective to start pursuing."
                    }),
                ),
                (
                    "token_budget",
                    serde_json::json!({
                        "type": "integer",
                        "description": "Optional positive token budget for the new active goal."
                    }),
                ),
            ],
            vec!["objective"],
        ),
        execute: Arc::new(move |_tool_call_id, args, _ctx: ToolContext| {
            let gm = goal_manager.clone();
            Box::pin(async move {
                let objective = args.get("objective")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing 'objective' argument".to_string())?;

                let token_budget = args.get("token_budget").and_then(|v| v.as_i64());

                match gm.create(objective.to_string(), token_budget) {
                    Ok(goal) => {
                        let resp = serde_json::json!({
                            "goal": goal_entry_to_json(&goal),
                            "message": "Goal created. Use get_goal to check current status."
                        });
                        Ok(AgentToolResult {
                            content: vec![ContentBlock::text(
                                serde_json::to_string_pretty(&resp).unwrap_or_default()
                            )],
                            is_error: false,
                            terminate: false,
                        })
                    }
                    Err(e) => Err(e),
                }
            })
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}

pub fn create_update_goal_tool(goal_manager: Arc<GoalManager>) -> AgentTool {
    AgentTool {
        name: "update_goal".to_string(),
        description: "Update the existing goal. Use this tool only to mark the goal achieved or genuinely blocked."
            .to_string(),
        prompt_snippet: Some("Update current thread goal".to_string()),
        prompt_guidelines: vec![],
        label: "update_goal".to_string(),
        parameters: make_params(
            "Update the existing goal. Only marks complete, blocked, or budget_limited.",
            vec![(
                "status",
                serde_json::json!({
                    "type": "string",
                    "enum": ["complete", "blocked"],
                    "description": "Set to `complete` only when the objective is achieved. \
                     Set to `blocked` only when genuinely stuck."
                }),
            )],
            vec!["status"],
        ),
        execute: Arc::new(move |_tool_call_id, args, _ctx: ToolContext| {
            let gm = goal_manager.clone();
            Box::pin(async move {
                let status = args.get("status")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing 'status' argument".to_string())?;

                if status != "complete" && status != "blocked" {
                    return Err("update_goal can only mark the existing goal complete or blocked".to_string());
                }

                match gm.update_status(status.to_string()) {
                    Ok(goal) => {
                        let resp = serde_json::json!({
                            "goal": goal_entry_to_json(&goal),
                            "message": format!("Goal marked as {}.", status),
                        });
                        Ok(AgentToolResult {
                            content: vec![ContentBlock::text(
                                serde_json::to_string_pretty(&resp).unwrap_or_default()
                            )],
                            is_error: false,
                            terminate: false,
                        })
                    }
                    Err(e) => Err(e),
                }
            })
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}
