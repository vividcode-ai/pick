use std::sync::Arc;

use pick_ai::types::ContentBlock;

use crate::core::state::{AgentTool, AgentToolResult, ToolContext, ToolExecutionMode};
use crate::session::goal::GoalManager;

fn make_params(
    desc: &str,
    props: Vec<(&str, serde_json::Value)>,
    required: Vec<&str>,
) -> pick_ai::types::JsonSchema {
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
        "completionCriterion": goal.completion_criterion,
        "status": goal.status,
        "tokenBudget": goal.token_budget,
        "tokensUsed": goal.tokens_used,
        "timeUsedSeconds": goal.time_used_seconds,
        "createdAt": goal.created_at,
        "updatedAt": goal.updated_at,
    })
}

/// Determine whether the current execution context is a sub-agent.
fn is_sub_agent(ctx: &ToolContext) -> bool {
    ctx.agent_id.is_some()
}

/// Resolve the correct GoalManager: sub-agents target the parent session's manager.
fn resolve_goal_manager<'a>(
    own: &'a Arc<GoalManager>,
    parent: &'a Option<Arc<GoalManager>>,
    is_sub: bool,
) -> Option<&'a Arc<GoalManager>> {
    if is_sub { parent.as_ref() } else { Some(own) }
}

/// Create a stub goal tool (no real GoalManager).
/// Returns canned responses when no goal system is available.
pub fn create_goal_tool_stub() -> AgentTool {
    AgentTool {
        name: "goal".to_string(),
        description: "Manage the active goal. Use `op` parameter: \
            create (requires objective + completion_criterion), get, \
            complete, pause, resume, cancel."
            .to_string(),
        prompt_snippet: Some("Manage the active goal".to_string()),
        prompt_guidelines: vec![],
        usage_example: None,
        label: "goal".to_string(),
        parameters: make_params(
            "Manage the active goal-mode objective.",
            vec![
                (
                    "op",
                    serde_json::json!({
                        "type": "string",
                        "enum": ["create", "get", "complete", "pause", "resume", "cancel"],
                        "description": "Goal operation"
                    }),
                ),
                (
                    "objective",
                    serde_json::json!({
                        "type": "string",
                        "description": "Goal objective (required for create)"
                    }),
                ),
                (
                    "completion_criterion",
                    serde_json::json!({
                        "type": "string",
                        "description": "Concrete checkable completion conditions (required for create)"
                    }),
                ),
                (
                    "token_budget",
                    serde_json::json!({
                        "type": "integer",
                        "description": "Optional positive token budget for the goal"
                    }),
                ),
            ],
            vec!["op"],
        ),
        execute: Arc::new(move |_tool_call_id, _args, _ctx: ToolContext| {
            Box::pin(async move {
                Ok(AgentToolResult {
                    content: vec![ContentBlock::text(
                        "{\"goal\": null, \"message\": \"No active goal for this thread.\"}",
                    )],
                    is_error: false,
                    terminate: false,
                })
            })
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}

/// Create the real goal tool wired to a GoalManager.
/// Handles 6 operations with sub-agent permission isolation.
pub fn create_goal_tool(goal_manager: Arc<GoalManager>) -> AgentTool {
    AgentTool {
        name: "goal".to_string(),
        description: "Manage the active goal. Use `op` parameter: \
            create (requires objective + completion_criterion), get, \
            complete, pause, resume, cancel."
            .to_string(),
        prompt_snippet: Some("Manage the active goal".to_string()),
        prompt_guidelines: vec![],
        usage_example: None,
        label: "goal".to_string(),
        parameters: make_params(
            "Manage the active goal-mode objective.",
            vec![
                (
                    "op",
                    serde_json::json!({
                        "type": "string",
                        "enum": ["create", "get", "complete", "pause", "resume", "cancel"],
                        "description": "Goal operation"
                    }),
                ),
                (
                    "objective",
                    serde_json::json!({
                        "type": "string",
                        "description": "Goal objective (required for create)"
                    }),
                ),
                (
                    "completion_criterion",
                    serde_json::json!({
                        "type": "string",
                        "description": "Concrete checkable completion conditions (required for create)"
                    }),
                ),
                (
                    "token_budget",
                    serde_json::json!({
                        "type": "integer",
                        "description": "Optional positive token budget for the goal"
                    }),
                ),
            ],
            vec!["op"],
        ),
        execute: Arc::new(move |_tool_call_id, args, ctx: ToolContext| {
            let gm = goal_manager.clone();
            let parent_gm = ctx.parent_goal_manager.clone();
            Box::pin(async move {
                let op = args
                    .get("op")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing 'op' argument. Use one of: create, get, complete, pause, resume, cancel.".to_string())?;

                let sub = is_sub_agent(&ctx);
                let target_gm = resolve_goal_manager(&gm, &parent_gm, sub);

                match op {
                    "create" => {
                        if sub {
                            return Err("Sub-agents cannot create goals.".to_string());
                        }
                        let objective =
                            args.get("objective")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    "Missing 'objective' argument (required for create)".to_string()
                                })?;
                        let criterion = args
                            .get("completion_criterion")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let token_budget = args.get("token_budget").and_then(|v| v.as_i64());
                        match gm.create(objective.to_string(), criterion.to_string(), token_budget)
                        {
                            Ok(goal) => {
                                let resp = serde_json::json!({
                                    "goal": goal_entry_to_json(&goal),
                                    "message": "Goal created."
                                });
                                Ok(AgentToolResult {
                                    content: vec![ContentBlock::text(
                                        serde_json::to_string_pretty(&resp).unwrap_or_default(),
                                    )],
                                    is_error: false,
                                    terminate: false,
                                })
                            }
                            Err(e) => Err(e),
                        }
                    }

                    "get" => {
                        let target =
                            target_gm.ok_or_else(|| "No goal manager available".to_string())?;
                        match target.get() {
                            Some(goal) => {
                                let remaining = target.remaining_tokens();
                                let mut resp = serde_json::json!({
                                    "goal": goal_entry_to_json(&goal),
                                });
                                if let Some(r) = remaining {
                                    resp["remainingTokens"] = serde_json::json!(r);
                                }
                                Ok(AgentToolResult {
                                    content: vec![ContentBlock::text(
                                        serde_json::to_string_pretty(&resp).unwrap_or_default(),
                                    )],
                                    is_error: false,
                                    terminate: false,
                                })
                            }
                            None => Ok(AgentToolResult {
                                content: vec![ContentBlock::text(
                                    "{\"goal\": null, \"message\": \"No active goal.\"}",
                                )],
                                is_error: false,
                                terminate: false,
                            }),
                        }
                    }

                    "complete" => {
                        if sub {
                            // Sub-agent: allowed — complete the parent session's goal
                            let target =
                                target_gm.ok_or_else(|| "No parent goal manager".to_string())?;
                            let goal = target
                                .get()
                                .ok_or_else(|| "No active goal to complete.".to_string())?;
                            if goal.status != "active" {
                                return Err(format!(
                                    "Goal is not active (status: {}). Cannot complete.",
                                    goal.status
                                ));
                            }
                            let _ = target.update_status("complete".to_string())?;
                            Ok(AgentToolResult {
                                content: vec![ContentBlock::text(format!(
                                    "Goal completed: \"{}\"",
                                    goal.objective
                                ))],
                                is_error: false,
                                terminate: true,
                            })
                        } else {
                            // Main session: blocked → must use goal-verify sub-agent
                            let goal = gm.get().ok_or_else(|| "No active goal.".to_string())?;
                            if goal.status != "active" {
                                return Err(format!(
                                    "Goal is not active (status: {}).",
                                    goal.status
                                ));
                            }
                            return Err("BLOCKED: Direct completion is not allowed. \
                                Use the `subagent` tool with agent `goal-verify` \
                                and provide a task description of what to verify. \
                                The goal-verify agent will independently inspect \
                                the work and call goal(op:\"complete\") if satisfied."
                                .to_string());
                        }
                    }

                    "pause" => {
                        if sub {
                            return Err("Sub-agents cannot pause goals.".to_string());
                        }
                        match gm.set_paused() {
                            Ok(goal) => {
                                let resp = serde_json::json!({
                                    "goal": goal_entry_to_json(&goal),
                                    "message": "Goal paused."
                                });
                                Ok(AgentToolResult {
                                    content: vec![ContentBlock::text(
                                        serde_json::to_string_pretty(&resp).unwrap_or_default(),
                                    )],
                                    is_error: false,
                                    terminate: false,
                                })
                            }
                            Err(e) => Err(e),
                        }
                    }

                    "resume" => {
                        if sub {
                            return Err("Sub-agents cannot resume goals.".to_string());
                        }
                        match gm.set_active() {
                            Ok(goal) => {
                                let resp = serde_json::json!({
                                    "goal": goal_entry_to_json(&goal),
                                    "message": "Goal resumed."
                                });
                                Ok(AgentToolResult {
                                    content: vec![ContentBlock::text(
                                        serde_json::to_string_pretty(&resp).unwrap_or_default(),
                                    )],
                                    is_error: false,
                                    terminate: false,
                                })
                            }
                            Err(e) => Err(e),
                        }
                    }

                    "cancel" => {
                        if sub {
                            return Err("Sub-agents cannot cancel goals.".to_string());
                        }
                        gm.clear().map_err(|e| e.to_string())?;
                        Ok(AgentToolResult {
                            content: vec![ContentBlock::text("Goal cancelled.")],
                            is_error: false,
                            terminate: false,
                        })
                    }

                    _ => Err(format!(
                        "Unknown op '{}'. Use one of: create, get, complete, pause, resume, cancel.",
                        op
                    )),
                }
            })
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}
