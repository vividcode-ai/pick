//! Subagent runner - agent execution logic

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use pick_ai::models::get_model;
use pick_ai::types::{ContentBlock, JsonSchema, Message, Model, UserMessage};

use crate::agent_config::{AgentConfig, AgentSource, discover_agents, format_agent_list};
use crate::agent_registry::AgentRegistry;
use crate::core::agent_loop::AgentLoopConfig;
use crate::core::events::{AgentEvent, AgentEventHandler};
use crate::core::hooks::{ToolEvent, WaitingKind};
use crate::core::state::{AgentTool, AgentToolResult, ToolContext, ToolExecutionMode};
use crate::inter_agent::AgentStatus;
use crate::permission::Ruleset;
use crate::permission::manager::PermissionManager;
use crate::permission::sandbox::Sandbox as SandboxTrait;
use crate::tools::registry::{create_coding_tools, create_coding_tools_with_goal_manager};

use super::stats::{SingleResult, SubagentStats};

const MAX_PARALLEL_TASKS: usize = 8;
const MAX_CONCURRENCY: usize = 4;

fn agent_source_str(agent: &AgentConfig) -> &str {
    match agent.source {
        AgentSource::User => "user",
        AgentSource::Project => "project",
        AgentSource::Builtin => "builtin",
    }
}

fn build_subagent_loop_config(
    model: Model,
    agent: &AgentConfig,
    tools: Vec<AgentTool>,
    child_event_handler: AgentEventHandler,
    fs_policy: Option<std::sync::Arc<crate::permission::fs_policy::FileSystemPolicy>>,
    cwd: Option<std::path::PathBuf>,
    _permission_manager: Option<Arc<PermissionManager>>,
    _mode_rulesets: Option<Vec<Ruleset>>,
    sandbox: Option<Arc<dyn SandboxTrait>>,
    sandbox_enabled: Option<Arc<AtomicBool>>,
    get_api_key: Option<Arc<dyn Fn() -> Option<String> + Send + Sync>>,
    agent_id: Option<String>,
    parent_goal_manager: Option<Arc<crate::session::goal::GoalManager>>,
    _tool_execution_permission: Option<String>,
) -> AgentLoopConfig {
    AgentLoopConfig {
        model,
        system_prompt: agent.system_prompt.clone(),
        developer_sections: vec![],
        tools,
        thinking_level: crate::core::state::ThinkingLevel::Off,
        max_tokens: None,
        temperature: None,
        extension_runner: None,
        transform_context: None,
        get_api_key,
        before_tool_call: None,
        should_stop_after_turn: None,
        get_steering_messages: None,
        get_follow_up_messages: None,
        provider_max_retries: None,
        provider_max_retry_delay_ms: None,
        approve: None,
        question: None,
        agent_id,
        agent_registry: None,
        on_turn_complete: None,
        on_event: Some(child_event_handler),
        fs_policy,
        cwd,
        // Subagents are sandboxed by fs_policy and filtered tools.
        // Inheriting the parent's permission_manager would cause tool
        // calls to get blocked (no interactive approval in a subagent).
        permission_hooks: None,
        mode_rulesets: None,
        tool_event_bus: None,
        permission_manager: None,
        tool_execution_permission: Some("auto_approve".to_string()),
        sandbox,
        sandbox_enabled,
        cancel_signal_tx: None,
        skill_paths: Vec::new(),
        parent_goal_manager,
    }
}

fn setup_subagent_session(
    result: &mut SingleResult,
    registry: Option<&Arc<AgentRegistry>>,
    parent_model: Option<Model>,
) -> Option<(Arc<AgentRegistry>, Model)> {
    let registry = match registry {
        Some(r) => r.clone(),
        None => {
            result.exit_code = 1;
            result.error = "Agent registry not available".to_string();
            return None;
        }
    };

    // Resolve the model: inherit from parent, or fall back to env var defaults
    let model = match parent_model {
        Some(m) => m,
        None => {
            let provider =
                std::env::var("pick_DEFAULT_PROVIDER").unwrap_or_else(|_| "deepseek".to_string());
            let model_id = std::env::var("pick_DEFAULT_MODEL")
                .unwrap_or_else(|_| "deepseek-v4-flash".to_string());
            match get_model(&provider, &model_id) {
                Some(m) => m,
                None => {
                    result.exit_code = 1;
                    result.error = format!(
                        "Default model '{}' not found for provider '{}'. \
                         Set pick_DEFAULT_PROVIDER and pick_DEFAULT_MODEL env vars, \
                         or configure default_provider/default_model in settings.",
                        model_id, provider
                    );
                    return None;
                }
            }
        }
    };

    Some((registry, model))
}

async fn run_subagent_turn(
    agent: &AgentConfig,
    task: &str,
    _cwd: &std::path::Path,
    progress: Option<&tokio::sync::mpsc::UnboundedSender<String>>,
    _agent_mode: Option<&str>,
    registry: Arc<AgentRegistry>,
    parent_agent_id: &str,
    model: Model,
    fs_policy: Option<std::sync::Arc<crate::permission::fs_policy::FileSystemPolicy>>,
    cwd: Option<std::path::PathBuf>,
    permission_manager: Option<Arc<PermissionManager>>,
    mode_rulesets: Option<Vec<Ruleset>>,
    sandbox: Option<Arc<dyn SandboxTrait>>,
    get_api_key: Option<Arc<dyn Fn() -> Option<String> + Send + Sync>>,
    parent_goal_manager: Option<Arc<crate::session::goal::GoalManager>>,
    tool_execution_permission: Option<String>,
) -> SingleResult {
    let mut result = SingleResult {
        agent: agent.name.clone(),
        ..Default::default()
    };

    // Build tool set with real goal tool wired to parent's GoalManager
    let mut tools = if let Some(ref pgm) = parent_goal_manager {
        create_coding_tools_with_goal_manager(None, pgm.clone())
    } else {
        create_coding_tools()
    };
    tools.retain(|t| t.name != "subagent");
    if let Some(ref tool_names) = agent.tools
        && !tool_names.is_empty()
    {
        tools.retain(|t| tool_names.contains(&t.name));
    }

    // Capture output text and usage from child's on_event
    let output = Arc::new(Mutex::new(String::new()));
    let stats = Arc::new(Mutex::new(SubagentStats::default()));

    let on_event_output = output.clone();
    let on_event_stats = stats.clone();
    let on_event_progress = progress.cloned();

    let child_event_handler: AgentEventHandler = Arc::new(move |event| {
        // Only send progress from MessageEnd. Ignore all intermediate
        // updates (MessageUpdate, ToolExecutionStart/End) — only the
        // final reply text is shown to the user.
        if let AgentEvent::MessageEnd { message } = &event
            && let Message::Assistant(msg) = message
        {
            // Extract final text content and send it as one update
            let mut out = on_event_output.lock().unwrap();
            let mut final_text = String::new();
            for block in &msg.content {
                if let ContentBlock::Text(t) = block {
                    out.push_str(&t.text);
                    out.push('\n');
                    final_text.push_str(&t.text);
                    final_text.push('\n');
                }
            }
            // Send final text once (will display in tool_end output)
            if let Some(ref tx) = on_event_progress {
                let _ = tx.send(final_text.trim().to_string());
            }
            // Track usage
            let s = &mut *on_event_stats.lock().unwrap();
            s.input += msg.usage.input;
            s.output += msg.usage.output;
            s.cache_read += msg.usage.cache_read;
            s.cache_write += msg.usage.cache_write;
            s.model = Some(msg.model.clone());
            s.stop_reason = Some(format!("{:?}", msg.stop_reason));
            s.error_message = msg.error_message.clone();
            s.turns += 1;
        }
    });

    // Build the child agent's configuration — inherit permission context from parent
    let child_config = build_subagent_loop_config(
        model,
        agent,
        tools,
        child_event_handler,
        fs_policy,
        cwd,
        permission_manager,
        mode_rulesets,
        sandbox,
        None, // subagents inherit parent sandbox_enabled via ToolContext
        get_api_key,
        Some(agent.name.clone()),
        parent_goal_manager,
        tool_execution_permission,
    );

    let initial_messages = vec![Message::User(UserMessage::text(format!("Task: {}", task)))];

    // Spawn the child agent in-process
    let agent_name = agent.name.clone();
    let child_result = registry
        .spawn_child(&agent_name, child_config, initial_messages, parent_agent_id)
        .await;

    let (child_id, child) = match child_result {
        Ok((id, child)) => (id, child),
        Err(e) => {
            result.exit_code = 1;
            result.error = format!("Failed to spawn child agent: {}", e);
            return result;
        }
    };

    // Wait for completion via status watch channel
    let mut status_rx = child.status_rx.clone();
    let mut final_status = AgentStatus::Running;
    loop {
        if status_rx.changed().await.is_err() {
            break;
        }
        let status = status_rx.borrow().clone();
        if status == AgentStatus::Completed || matches!(status, AgentStatus::Errored(_)) {
            final_status = status;
            break;
        }
    }

    match final_status {
        AgentStatus::Completed => {
            result.exit_code = 0;
        }
        AgentStatus::Errored(e) => {
            result.exit_code = 1;
            result.error = e;
        }
        _ => {
            result.exit_code = 1;
            result.error = "Child agent did not complete".to_string();
        }
    }

    // Collect output and stats
    result.output = output.lock().unwrap().clone();
    {
        let s = stats.lock().unwrap();
        result.stats = SubagentStats {
            input: s.input,
            output: s.output,
            cache_read: s.cache_read,
            cache_write: s.cache_write,
            model: s.model.clone(),
            stop_reason: s.stop_reason.clone(),
            error_message: s.error_message.clone(),
            turns: s.turns,
        };
    } // MutexGuard dropped here, before any .await

    // Clean up registry
    registry.remove(&child_id).await;

    result
}

/// Run a single subagent in-process using AgentRegistry::spawn_child.
/// No subprocess spawning, no pipe deadlocks.
async fn run_single_agent(
    agent: &AgentConfig,
    task: &str,
    _cwd: &std::path::Path,
    progress: Option<&tokio::sync::mpsc::UnboundedSender<String>>,
    _agent_mode: Option<&str>,
    registry: Option<&Arc<AgentRegistry>>,
    parent_agent_id: &str,
    parent_model: Option<Model>,
    get_api_key: Option<Arc<dyn Fn() -> Option<String> + Send + Sync>>,
    fs_policy: Option<std::sync::Arc<crate::permission::fs_policy::FileSystemPolicy>>,
    cwd: Option<std::path::PathBuf>,
    permission_manager: Option<Arc<PermissionManager>>,
    mode_rulesets: Option<Vec<Ruleset>>,
    sandbox: Option<Arc<dyn SandboxTrait>>,
    parent_goal_manager: Option<Arc<crate::session::goal::GoalManager>>,
    tool_execution_permission: Option<String>,
) -> SingleResult {
    let mut result = SingleResult {
        agent: agent.name.clone(),
        ..Default::default()
    };

    let (registry, model) = match setup_subagent_session(&mut result, registry, parent_model) {
        Some(v) => v,
        None => return result,
    };

    run_subagent_turn(
        agent,
        task,
        _cwd,
        progress,
        _agent_mode,
        registry,
        parent_agent_id,
        model,
        fs_policy,
        cwd,
        permission_manager,
        mode_rulesets,
        sandbox,
        get_api_key,
        parent_goal_manager,
        tool_execution_permission,
    )
    .await
}

/// Execute a single subagent with event handling (progress, output formatting)
async fn execute_subagent_with_events(
    agent: &AgentConfig,
    task: &str,
    task_cwd: &std::path::Path,
    ctx: &ToolContext,
    agent_mode: Option<&str>,
) -> Result<AgentToolResult, String> {
    if let Some(ref tx) = ctx.progress {
        let _ = tx.send(format!(
            "[subagent] Starting agent: {} ({})\n",
            agent.name,
            agent_source_str(agent)
        ));
    }

    let parent_id = ctx.agent_id.as_deref().unwrap_or("root");
    let parent_goal_manager = ctx.parent_goal_manager.clone();
    let result = run_single_agent(
        agent,
        task,
        task_cwd,
        ctx.progress.as_ref(),
        agent_mode,
        ctx.agent_registry.as_ref(),
        parent_id,
        ctx.default_model.clone(),
        ctx.get_api_key.clone(),
        ctx.fs_policy.clone(),
        ctx.cwd.clone(),
        ctx.permission_manager.clone(),
        ctx.permission_manager.as_ref().map(|_| Vec::new()),
        ctx.sandbox.clone(),
        parent_goal_manager,
        ctx.tool_execution_permission.clone(),
    )
    .await;

    if result.exit_code != 0 {
        return Ok(AgentToolResult {
            content: vec![ContentBlock::text(format!(
                "Agent {} failed (exit code: {}):\n{}",
                agent.name,
                result.exit_code,
                if result.error.is_empty() {
                    result.output
                } else {
                    result.error
                }
            ))],
            is_error: true,
            terminate: false,
        });
    }

    let usage_footer = if result.stats.turns > 0 {
        let s = &result.stats;
        format!(
            "\n\n---\n*Turns: {} | Input: {} | Output: {} | Cache R/W: {}/{} | Model: {}*",
            s.turns,
            s.input,
            s.output,
            s.cache_read,
            s.cache_write,
            s.model.as_deref().unwrap_or("default")
        )
    } else {
        String::new()
    };

    Ok(AgentToolResult {
        content: vec![ContentBlock::text(format!(
            "{}{}",
            result.output, usage_footer
        ))],
        is_error: false,
        terminate: false,
    })
}

/// Aggregate subagent results into a formatted output string
fn aggregate_subagent_results(results: &[SingleResult]) -> (String, bool) {
    let success_count = results.iter().filter(|r| r.exit_code == 0).count();
    let mut output = format!(
        "Parallel: {}/{} succeeded\n\n",
        success_count,
        results.len()
    );

    for r in results {
        let status = if r.exit_code == 0 {
            "completed"
        } else {
            "failed"
        };
        let usage_str = format_usage_summary(&r.stats);
        output.push_str(&format!(
            "### [{}] {}\n\n{}\n{}\n\n---\n\n",
            r.agent,
            status,
            if r.error.is_empty() {
                &r.output
            } else {
                &r.error
            },
            usage_str
        ));
    }

    (output, success_count < results.len())
}

/// Execute parallel tasks with concurrency limit using Arc-based shared state
async fn run_parallel_agents(
    tasks: Vec<(AgentConfig, String, std::path::PathBuf)>,
    concurrency: usize,
    agent_mode: Option<String>,
    registry: Option<Arc<AgentRegistry>>,
    parent_id: String,
    parent_model: Option<Model>,
    parent_get_api_key: Option<Arc<dyn Fn() -> Option<String> + Send + Sync>>,
    parent_tool_execution_permission: Option<String>,
) -> Vec<SingleResult> {
    if tasks.is_empty() {
        return Vec::new();
    }
    let total = tasks.len();
    let limit = concurrency.max(1).min(total);
    let results = std::sync::Arc::new(std::sync::Mutex::new(Vec::with_capacity(total)));
    let index = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let tasks = std::sync::Arc::new(tasks);
    let agent_mode = std::sync::Arc::new(agent_mode);
    let registry = registry.map(std::sync::Arc::new);
    let parent_model = parent_model.map(std::sync::Arc::new);
    let parent_get_api_key = parent_get_api_key.map(std::sync::Arc::new);
    let parent_tool_execution_permission =
        parent_tool_execution_permission.map(std::sync::Arc::new);

    let mut handles = Vec::with_capacity(limit);
    for _ in 0..limit {
        let results = results.clone();
        let index = index.clone();
        let tasks = tasks.clone();
        let agent_mode = agent_mode.clone();
        let registry = registry.clone();
        let parent_id = parent_id.clone();
        let parent_model = parent_model.clone();
        let parent_get_api_key = parent_get_api_key.clone();
        let parent_tool_execution_permission = parent_tool_execution_permission.clone();
        handles.push(tokio::spawn(async move {
            loop {
                let i = index.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as usize;
                if i >= total {
                    break;
                }
                let (ref agent, ref task, ref cwd) = (*tasks)[i];
                let model = parent_model.as_ref().map(|m| (**m).clone());
                let result = run_single_agent(
                    agent,
                    task,
                    cwd,
                    None,
                    agent_mode.as_deref(),
                    registry.as_deref(),
                    &parent_id,
                    model,
                    parent_get_api_key.as_deref().cloned(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    parent_tool_execution_permission.as_deref().cloned(),
                )
                .await;
                let mut guard = results.lock().unwrap();
                guard.push(result);
            }
        }));
    }

    for h in handles {
        let _ = h.await;
    }

    results.lock().unwrap().clone()
}

/// Format usage statistics as a single-line summary string
fn format_usage_summary(stats: &SubagentStats) -> String {
    if stats.turns == 0 {
        return String::new();
    }
    format!(
        "*Turns: {} | Input: {} | Output: {} | Cache R/W: {}/{} | Model: {}*",
        stats.turns,
        stats.input,
        stats.output,
        stats.cache_read,
        stats.cache_write,
        stats.model.as_deref().unwrap_or("default")
    )
}

/// Create the subagent tool definition
pub fn create_subagent_tool() -> AgentTool {
    create_subagent_tool_with_mode(None)
}

/// Create subagent tool with optional agent mode for child process inheritance
pub fn create_subagent_tool_with_mode(agent_mode: Option<String>) -> AgentTool {
    let mode = agent_mode.clone();
    let params = JsonSchema {
        schema_type: "object".to_string(),
        properties: Some(
            vec![
                (
                    "agent".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Name of the agent to invoke (for single mode)"
                    }),
                ),
                (
                    "task".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Task to delegate (for single mode)"
                    }),
                ),
                (
                    "tasks".to_string(),
                    serde_json::json!({
                        "type": "array",
                        "description": "Array of {agent, task} for parallel execution",
                        "items": {
                            "type": "object",
                            "properties": {
                                "agent": { "type": "string" },
                                "task": { "type": "string" },
                                "cwd": { "type": "string" }
                            },
                            "required": ["agent", "task"]
                        }
                    }),
                ),
                (
                    "chain".to_string(),
                    serde_json::json!({
                        "type": "array",
                        "description": "Array of {agent, task} for sequential execution; {previous} is replaced with prior agent's output",
                        "items": {
                            "type": "object",
                            "properties": {
                                "agent": { "type": "string" },
                                "task": { "type": "string" },
                                "cwd": { "type": "string" }
                            },
                            "required": ["agent", "task"]
                        }
                    }),
                ),
                (
                    "agent_scope".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "enum": ["user", "project", "both"],
                        "description": "Which agent directories to use. Default: both",
                        "default": "both"
                    }),
                ),
                (
                    "cwd".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Working directory for the agent process (single mode)"
                    }),
                ),
                (
                    "confirm_project_agents".to_string(),
                    serde_json::json!({
                        "type": "boolean",
                        "description": "Prompt before running project-local agents. Default: true",
                    }),
                ),
            ]
            .into_iter()
            .collect(),
        ),
        required: Some(vec![]),
        description: Some("Delegate tasks to specialized subagents with isolated context. Modes: single (agent + task), parallel (tasks array), chain (sequential with {previous} placeholder).".to_string()),
        items: None,
        additional_properties: Some(false),
    };

    AgentTool {
        name: "subagent".to_string(),
        description: "Delegate tasks to specialized subagents with isolated context. Supports single, parallel, and chain modes.".to_string(),
        prompt_snippet: Some("Use subagent to delegate tasks to specialized agents".to_string()),
        prompt_guidelines: vec![
            "Single mode: { agent: \"scout\", task: \"find auth code\" }".to_string(),
            "Parallel: { tasks: [{ agent: \"scout\", task: \"...\" }, ...] }".to_string(),
            "Chain: { chain: [{ agent: \"scout\", task: \"...\" }, { agent: \"planner\", task: \"{previous}\" }] }".to_string(),
            "Available agents and their descriptions will be listed in the response.".to_string(),
            "When you need to execute tasks in parallel or handle multiple independent subtasks simultaneously, you MUST use the subagent tool with the tasks array (parallel mode) rather than processing them sequentially yourself.".to_string(),
        ],
        usage_example: Some(vec![
            r#"subagent(agent: "scout", task: "find auth implementation")"#.to_string(),
        ]),
        label: "Subagent".to_string(),
        parameters: params,
        execute: Arc::new({
            let mode = mode.clone();
            move |tool_call_id, args, ctx| {
                let mode = mode.clone();
                Box::pin(async move {
                    execute_subagent(args, ctx, tool_call_id, mode.as_deref()).await
                })
            }
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}

async fn execute_subagent(
    args: serde_json::Value,
    ctx: ToolContext,
    tool_call_id: String,
    agent_mode: Option<&str>,
) -> Result<AgentToolResult, String> {
    let agent_scope = args
        .get("agent_scope")
        .and_then(|v| v.as_str())
        .unwrap_or("both");

    let scope = match agent_scope {
        "project" => crate::agent_config::AgentScope::Project,
        "both" => crate::agent_config::AgentScope::Both,
        _ => crate::agent_config::AgentScope::User,
    };

    let cwd = args
        .get("cwd")
        .and_then(|v| v.as_str())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Resolve agent directory using the crate config
    let agent_dir = {
        let home = dirs::home_dir().unwrap_or_default();
        home.join(".pick").join("agent")
    };

    let discovery = discover_agents(&cwd, &agent_dir, &scope);

    // Confirm project-local agents if applicable
    if scope != crate::agent_config::AgentScope::User {
        let confirm = args
            .get("confirm_project_agents")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        if confirm {
            let project_agents: Vec<&AgentConfig> = discovery
                .agents
                .iter()
                .filter(|a| matches!(a.source, AgentSource::Project))
                .collect();
            if !project_agents.is_empty()
                && let Some(ref approve) = ctx.approve
            {
                // Publish WaitingForUser event before prompting
                if let Some(ref bus) = ctx.tool_event_bus {
                    bus.publish(&ToolEvent::WaitingForUser {
                        tool_name: "subagent".to_string(),
                        tool_call_id: tool_call_id.clone(),
                        input: args.clone(),
                        kind: WaitingKind::Permission {
                            permission: "subagent".to_string(),
                        },
                        summary: "Run project-local agents".to_string(),
                    })
                    .await;
                }
                let names: Vec<&str> = project_agents.iter().map(|a| a.name.as_str()).collect();
                let dir = discovery
                    .project_agents_dir
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "(unknown)".to_string());
                let ok = approve(
                        "Run project-local agents?".to_string(),
                        format!(
                            "Agents: {}\nSource: {}\n\nProject agents are repo-controlled. Only continue for trusted repositories.",
                            names.join(", "), dir
                        ),
                    ).await;
                if !ok {
                    return Ok(AgentToolResult {
                        content: vec![ContentBlock::text(
                            "Canceled: project-local agents not approved.",
                        )],
                        is_error: true,
                        terminate: false,
                    });
                }
            }
        }
    }

    let has_chain = args
        .get("chain")
        .and_then(|v| v.as_array())
        .is_some_and(|a| !a.is_empty());
    let has_tasks = args
        .get("tasks")
        .and_then(|v| v.as_array())
        .is_some_and(|a| !a.is_empty());
    let has_single = args.get("agent").and_then(|v| v.as_str()).is_some()
        && args.get("task").and_then(|v| v.as_str()).is_some();

    let mode_count = [has_chain, has_tasks, has_single]
        .iter()
        .filter(|&&b| b)
        .count();

    if mode_count != 1 {
        let (agent_text, remaining) = format_agent_list(&discovery.agents, 20);
        let suffix = if remaining > 0 {
            format!(" (and {} more)", remaining)
        } else {
            String::new()
        };
        return Ok(AgentToolResult {
            content: vec![ContentBlock::text(format!(
                "Invalid parameters: provide exactly one mode (single, parallel, or chain).\nAvailable agents: {}{}",
                agent_text, suffix
            ))],
            is_error: true,
            terminate: false,
        });
    }

    // Single mode
    if let (Some(agent_name), Some(task)) = (
        args.get("agent").and_then(|v| v.as_str()),
        args.get("task").and_then(|v| v.as_str()),
    ) {
        let task_cwd = args
            .get("cwd")
            .and_then(|v| v.as_str())
            .map(std::path::PathBuf::from)
            .unwrap_or(cwd);

        let agent = discovery
            .agents
            .iter()
            .find(|a| a.name == agent_name)
            .cloned();

        let agent = match agent {
            Some(a) => a,
            None => {
                let (agent_text, _) = format_agent_list(&discovery.agents, 50);
                return Ok(AgentToolResult {
                    content: vec![ContentBlock::text(format!(
                        "Unknown agent: \"{}\". Available agents: {}",
                        agent_name, agent_text
                    ))],
                    is_error: true,
                    terminate: false,
                });
            }
        };

        return execute_subagent_with_events(&agent, task, &task_cwd, &ctx, agent_mode).await;
    }

    // Parallel mode
    if let Some(tasks) = args.get("tasks").and_then(|v| v.as_array()) {
        if tasks.is_empty() {
            return Ok(AgentToolResult {
                content: vec![ContentBlock::text(
                    "No tasks provided for parallel execution.",
                )],
                is_error: true,
                terminate: false,
            });
        }

        if tasks.len() > MAX_PARALLEL_TASKS {
            return Ok(AgentToolResult {
                content: vec![ContentBlock::text(format!(
                    "Too many parallel tasks ({}). Max is {}.",
                    tasks.len(),
                    MAX_PARALLEL_TASKS
                ))],
                is_error: true,
                terminate: false,
            });
        }

        struct TaskDef {
            agent: String,
            task: String,
            cwd: std::path::PathBuf,
        }

        let task_defs: Vec<TaskDef> = tasks
            .iter()
            .filter_map(|t| {
                let agent = t.get("agent")?.as_str()?.to_string();
                let task = t.get("task")?.as_str()?.to_string();
                let cwd = t
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| cwd.clone());
                Some(TaskDef { agent, task, cwd })
            })
            .collect();

        let valid_defs: Vec<(AgentConfig, String, std::path::PathBuf)> = task_defs
            .into_iter()
            .filter_map(|td| {
                discovery
                    .agents
                    .iter()
                    .find(|a| a.name == td.agent)
                    .map(|a| (a.clone(), td.task, td.cwd))
            })
            .collect();

        if valid_defs.is_empty() {
            return Ok(AgentToolResult {
                content: vec![ContentBlock::text(
                    "No valid agent definitions found for parallel tasks.",
                )],
                is_error: true,
                terminate: false,
            });
        }

        if let Some(ref tx) = ctx.progress {
            let _ = tx.send(format!(
                "[subagent] Running {} parallel tasks...\n",
                valid_defs.len()
            ));
        }

        let results = run_parallel_agents(
            valid_defs,
            MAX_CONCURRENCY,
            agent_mode.map(|s| s.to_string()),
            ctx.agent_registry.clone(),
            ctx.agent_id.unwrap_or_else(|| "root".to_string()),
            ctx.default_model.clone(),
            ctx.get_api_key.clone(),
            ctx.tool_execution_permission.clone(),
        )
        .await;

        let (output, has_errors) = aggregate_subagent_results(&results);

        return Ok(AgentToolResult {
            content: vec![ContentBlock::text(output)],
            is_error: has_errors,
            terminate: false,
        });
    }

    // Chain mode
    if let Some(chain) = args.get("chain").and_then(|v| v.as_array()) {
        if chain.is_empty() {
            return Ok(AgentToolResult {
                content: vec![ContentBlock::text("No steps provided for chain execution.")],
                is_error: true,
                terminate: false,
            });
        }

        let mut previous_output = String::new();
        let mut chain_output = String::new();

        for (i, step) in chain.iter().enumerate() {
            let agent_name = match step.get("agent").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => {
                    return Ok(AgentToolResult {
                        content: vec![ContentBlock::text(format!(
                            "Chain step {}: missing agent name",
                            i + 1
                        ))],
                        is_error: true,
                        terminate: false,
                    });
                }
            };

            let task = match step.get("task").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => {
                    return Ok(AgentToolResult {
                        content: vec![ContentBlock::text(format!(
                            "Chain step {}: missing task",
                            i + 1
                        ))],
                        is_error: true,
                        terminate: false,
                    });
                }
            };

            let step_cwd = step
                .get("cwd")
                .and_then(|v| v.as_str())
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| cwd.clone());

            let task_with_context = task.replace("{previous}", &previous_output);

            let agent = discovery
                .agents
                .iter()
                .find(|a| a.name == agent_name)
                .cloned();

            let agent = match agent {
                Some(a) => a,
                None => {
                    return Ok(AgentToolResult {
                        content: vec![ContentBlock::text(format!(
                            "Chain step {}: unknown agent \"{}\"",
                            i + 1,
                            agent_name
                        ))],
                        is_error: true,
                        terminate: false,
                    });
                }
            };

            let result = execute_subagent_with_events(
                &agent,
                &task_with_context,
                &step_cwd,
                &ctx,
                agent_mode,
            )
            .await?;

            let r = match &result.content[0] {
                ContentBlock::Text(t) => t.text.clone(),
                _ => String::new(),
            };

            chain_output.push_str(&format!(
                "### Step {}/{}: {}\n\n{}\n\n",
                i + 1,
                chain.len(),
                agent.name,
                r
            ));

            if result.is_error {
                return Ok(AgentToolResult {
                    content: vec![ContentBlock::text(format!(
                        "Chain stopped at step {} ({})\n\n{}",
                        i + 1,
                        agent.name,
                        chain_output
                    ))],
                    is_error: true,
                    terminate: false,
                });
            }

            // Extract the output without usage footer for {previous} replacement
            let result_text = match &result.content[0] {
                ContentBlock::Text(t) => {
                    // Strip the usage footer for chaining
                    if let Some(pos) = t.text.rfind("\n\n---") {
                        t.text[..pos].to_string()
                    } else {
                        t.text.clone()
                    }
                }
                _ => String::new(),
            };
            previous_output = result_text;
        }

        return Ok(AgentToolResult {
            content: vec![ContentBlock::text(chain_output)],
            is_error: false,
            terminate: false,
        });
    }

    // No valid mode detected
    let (agent_text, remaining) = format_agent_list(&discovery.agents, 20);
    let suffix = if remaining > 0 {
        format!(" (and {} more)", remaining)
    } else {
        String::new()
    };
    Ok(AgentToolResult {
        content: vec![ContentBlock::text(format!(
            "Invalid parameters. Available agents: {}{}",
            agent_text, suffix
        ))],
        is_error: true,
        terminate: false,
    })
}
