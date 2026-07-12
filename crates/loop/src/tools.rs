//! Goal tool definitions for loop mode.
//!
//! Three tools exposed to the agent when a goal loop is active:
//! - `opencode_loop_goal_complete` — mark the goal as complete
//! - `opencode_loop_goal_blocked` — mark the goal as blocked
//! - `opencode_loop_goal_progress` — record progress
//!
//! These are separate from the built-in `goal` tool, and follow the
//! opencode-loop convention of exposing structured tools for the agent
//! to call during a goal-driven loop iteration.

use std::sync::Arc;

use pick_agent::core::state::{AgentTool, AgentToolResult, ToolContext, ToolExecutionMode};
use pick_ai::types::{ContentBlock, JsonSchema};
use tokio::sync::RwLock;

use crate::manager::LoopManager;
use crate::types::LoopJobStatus;

fn text_params(desc: &str, props: Vec<(&str, &str, &str)>) -> JsonSchema {
    let required: Vec<String> = props.iter().map(|(k, _, _)| k.to_string()).collect();
    JsonSchema {
        schema_type: "object".to_string(),
        properties: Some(
            props
                .into_iter()
                .map(|(k, t, d)| {
                    (
                        k.to_string(),
                        serde_json::json!({
                            "type": t,
                            "description": d,
                        }),
                    )
                })
                .collect(),
        ),
        required: Some(required),
        description: Some(desc.to_string()),
        items: None,
        additional_properties: Some(false),
    }
}

/// Find the active goal job in the LoopManager.
fn find_active_goal_job(mgr: &LoopManager) -> Option<(usize, String)> {
    for (i, job) in mgr.list().iter().enumerate() {
        if job.is_goal() && job.status == LoopJobStatus::Running {
            return Some((i, job.id.clone()));
        }
    }
    None
}

/// Tool: `opencode_loop_goal_complete`
///
/// Mark the current goal loop as complete. The agent should call this
/// when all acceptance criteria are satisfied.
pub fn create_goal_complete_tool(loop_manager: Arc<RwLock<LoopManager>>) -> AgentTool {
    AgentTool {
        name: "opencode_loop_goal_complete".to_string(),
        description: "Mark the current OpenCode Loop experimental goal as completed. ".to_string(),
        prompt_snippet: None,
        prompt_guidelines: vec![],
        usage_example: None,
        label: "loop-goal-complete".to_string(),
        parameters: text_params(
            "Mark the goal as complete with summary and evidence.",
            vec![
                ("summary", "string", "Summary of what was accomplished"),
                ("evidence", "string", "Specific evidence of completion"),
            ],
        ),
        execute: Arc::new(move |_tool_call_id, args, _ctx: ToolContext| {
            let lm = loop_manager.clone();
            Box::pin(async move {
                let summary = args
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Goal completed");
                let evidence = args.get("evidence").and_then(|v| v.as_str()).unwrap_or("");

                let mut mgr = lm.write().await;
                let result = if let Some((_, id)) = find_active_goal_job(&mgr) {
                    mgr.mark_done(&id);
                    if let Some(job) = mgr.get_mut(&id) {
                        job.goal_status = Some("completed".to_string());
                        job.goal_progress
                            .push(format!("COMPLETED: {} | Evidence: {}", summary, evidence));
                    }
                    let _ = mgr.save();
                    format!("Goal completed: {}", summary)
                } else {
                    "No active goal loop to complete.".to_string()
                };

                Ok(AgentToolResult {
                    content: vec![ContentBlock::text(result)],
                    is_error: false,
                    terminate: true,
                })
            })
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}

/// Tool: `opencode_loop_goal_blocked`
///
/// Mark the current goal loop as blocked when user input is required.
pub fn create_goal_blocked_tool(loop_manager: Arc<RwLock<LoopManager>>) -> AgentTool {
    AgentTool {
        name: "opencode_loop_goal_blocked".to_string(),
        description: "Mark the current OpenCode Loop experimental goal as blocked ".to_string(),
        prompt_snippet: None,
        prompt_guidelines: vec![],
        usage_example: None,
        label: "loop-goal-blocked".to_string(),
        parameters: text_params(
            "Mark the goal as blocked with the reason.",
            vec![
                ("reason", "string", "Why the goal is blocked"),
                ("needed", "string", "What is needed to unblock"),
            ],
        ),
        execute: Arc::new(move |_tool_call_id, args, _ctx: ToolContext| {
            let lm = loop_manager.clone();
            Box::pin(async move {
                let reason = args
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown reason");

                let mut mgr = lm.write().await;
                let result = if let Some((_, id)) = find_active_goal_job(&mgr) {
                    mgr.mark_done(&id); // blocked is a terminal state for the run
                    if let Some(job) = mgr.get_mut(&id) {
                        job.goal_status = Some("blocked".to_string());
                        job.status = LoopJobStatus::Paused; // paused waiting for user
                        job.goal_progress.push(format!("BLOCKED: {}", reason));
                    }
                    let _ = mgr.save();
                    format!("Goal blocked: {}", reason)
                } else {
                    "No active goal loop to block.".to_string()
                };

                Ok(AgentToolResult {
                    content: vec![ContentBlock::text(result)],
                    is_error: false,
                    terminate: true,
                })
            })
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}

/// Tool: `opencode_loop_goal_progress`
///
/// Record meaningful progress on the current goal without completing it.
pub fn create_goal_progress_tool(loop_manager: Arc<RwLock<LoopManager>>) -> AgentTool {
    AgentTool {
        name: "opencode_loop_goal_progress".to_string(),
        description: "Record meaningful progress on the current OpenCode Loop ".to_string(),
        prompt_snippet: None,
        prompt_guidelines: vec![],
        usage_example: None,
        label: "loop-goal-progress".to_string(),
        parameters: text_params(
            "Record progress on the goal.",
            vec![
                ("summary", "string", "What progress was made"),
                ("next", "string", "What will be done next"),
            ],
        ),
        execute: Arc::new(move |_tool_call_id, args, _ctx: ToolContext| {
            let lm = loop_manager.clone();
            Box::pin(async move {
                let summary = args
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Progress made");
                let next = args
                    .get("next")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Continue working");

                let mut mgr = lm.write().await;
                let result = if let Some((_, id)) = find_active_goal_job(&mgr) {
                    if let Some(job) = mgr.get_mut(&id) {
                        job.goal_progress
                            .push(format!("PROGRESS: {} | Next: {}", summary, next));
                        // Keep last 30 entries
                        if job.goal_progress.len() > 30 {
                            job.goal_progress.drain(0..job.goal_progress.len() - 30);
                        }
                    }
                    let _ = mgr.save();
                    format!("Progress recorded: {}", summary)
                } else {
                    "No active goal loop.".to_string()
                };

                Ok(AgentToolResult {
                    content: vec![ContentBlock::text(result)],
                    is_error: false,
                    terminate: false,
                })
            })
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}

/// Return all three loop goal tools.
pub fn create_loop_goal_tools(loop_manager: Arc<RwLock<LoopManager>>) -> Vec<AgentTool> {
    vec![
        create_goal_complete_tool(loop_manager.clone()),
        create_goal_blocked_tool(loop_manager.clone()),
        create_goal_progress_tool(loop_manager),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LoopJob;

    fn make_manager_with_goal() -> Arc<RwLock<LoopManager>> {
        let mgr = LoopManager::new("test.json".into());
        let mut goal_job = LoopJob::new_goal("g1".into(), "test goal".into(), vec![], vec![], 0);
        goal_job.status = LoopJobStatus::Running;
        let mut mgr = mgr;
        mgr.create(goal_job);
        Arc::new(RwLock::new(mgr))
    }

    #[tokio::test]
    async fn test_complete_tool() {
        let lm = make_manager_with_goal();
        let tool = create_goal_complete_tool(lm.clone());
        let args = serde_json::json!({
            "summary": "All done",
            "evidence": "Tests pass"
        });
        let result = (tool.execute)("call1".into(), args, ToolContext::default())
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(
            matches!(&result.content[0], ContentBlock::Text(t) if t.text.contains("Goal completed"))
        );

        // Verify the job is done
        let mgr = lm.read().await;
        let job = mgr.get("g1").unwrap();
        assert_eq!(job.status, LoopJobStatus::Done);
    }

    #[tokio::test]
    async fn test_blocked_tool() {
        let lm = make_manager_with_goal();
        let tool = create_goal_blocked_tool(lm.clone());
        let args = serde_json::json!({
            "reason": "Need user input",
            "needed": "API key"
        });
        let result = (tool.execute)("call2".into(), args, ToolContext::default())
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(
            matches!(&result.content[0], ContentBlock::Text(t) if t.text.contains("Goal blocked"))
        );
    }

    #[tokio::test]
    async fn test_progress_tool() {
        let lm = make_manager_with_goal();
        let tool = create_goal_progress_tool(lm.clone());
        let args = serde_json::json!({
            "summary": "Fixed the parser",
            "next": "Add tests"
        });
        let result = (tool.execute)("call3".into(), args, ToolContext::default())
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(
            matches!(&result.content[0], ContentBlock::Text(t) if t.text.contains("Progress recorded"))
        );
    }

    #[tokio::test]
    async fn test_no_active_goal() {
        let lm = Arc::new(RwLock::new(LoopManager::new("test.json".into())));
        let tool = create_goal_complete_tool(lm);
        let args = serde_json::json!({"summary": "done", "evidence": ""});
        let result = (tool.execute)("call4".into(), args, ToolContext::default())
            .await
            .unwrap();
        assert!(
            matches!(&result.content[0], ContentBlock::Text(t) if t.text.contains("No active goal"))
        );
    }
}
