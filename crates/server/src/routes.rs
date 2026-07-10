use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::Event;

use pick_agent::core::agent_loop::AgentLoopConfig;
use pick_agent::core::state::ThinkingLevel;
use pick_agent::permission::fs_policy::FileSystemPolicy;
use pick_agent::permission::manager::PermissionManager;
use pick_agent::permission::{Action, Rule, Ruleset};
use pick_ai::types::Message as AiMessage;
use pick_ai::types::UserMessage;
use serde::Deserialize;
use tokio::sync::watch;
use tracing::{debug, error, info, warn};
use utoipa::ToSchema;

use crate::AppState;
use crate::approval::SseApprovalHook;
use crate::events;
use crate::git::get_git_info;
use crate::session::SseSessionState;
use axum::extract::Path;
use pick_agent::agent_registry::AgentRegistry;
use pick_agent::command::review::REVIEW_SYSTEM_PROMPT;

#[derive(Deserialize, ToSchema)]
pub struct AskRequest {
    pub session_id: String,
    pub prompt: String,
    pub thinking_level: Option<String>,
    pub extra_mode: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct CancelRequest {
    pub session_id: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ApproveRequest {
    pub session_id: String,
    pub approval_id: String,
    pub approved: bool,
}

#[derive(Deserialize, ToSchema)]
pub struct AnswerQuestionRequest {
    pub session_id: String,
    pub question_id: String,
    pub answers: Vec<Vec<String>>,
}

/// Submit a prompt to an agent session.
/// The message is enqueued and processed sequentially — if an agent loop is
/// already running, it will be picked up between turns. If not, a new loop starts.
#[utoipa::path(
    post,
    path = "/ask",
    tag = "agent",
    request_body = AskRequest,
    responses(
        (status = 202, description = "Message queued / Agent started"),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn ask(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AskRequest>,
) -> impl IntoResponse {
    let session = match state.session_manager.get(&req.session_id).await {
        Some(s) => s,
        None => {
            return (
                StatusCode::NOT_FOUND,
                format!("Session {} not found", req.session_id),
            )
                .into_response();
        }
    };

    let model = match pick_ai::models::get_model(&session.provider, &session.model_id) {
        Some(m) => m,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Model '{}' not found", session.model_id),
            )
                .into_response();
        }
    };

    // Look up the SSE session state
    let sse_state = {
        let sessions = state.sse_sessions.read().await;
        sessions.get(&req.session_id).cloned()
    };

    let sse_state = match sse_state {
        Some(s) => s,
        None => {
            return (
                StatusCode::PRECONDITION_FAILED,
                "No SSE connection for this session. Connect to /events/{session_id} first.",
            )
                .into_response();
        }
    };

    let agent_mode = sse_state.agent_mode.read().unwrap().clone();

    // Handle extra mode: if "goal", parse the prompt and create a goal
    if let Some(extra_mode) = &req.extra_mode {
        if extra_mode == "goal" {
            let (objective, criterion) = if let Some(pos) = req.prompt.find("||") {
                let obj = req.prompt[..pos].trim().to_string();
                let crit = req.prompt[pos + 2..].trim().to_string();
                (obj, crit)
            } else {
                (req.prompt.clone(), String::new())
            };
            let mut gm = sse_state.goal_manager.write().unwrap();
            let goal_manager =
                gm.get_or_insert_with(|| Arc::new(pick_agent::session::GoalManager::new()));
            if goal_manager.get().is_some() {
                warn!(
                    "Goal already exists for session {}, clearing first",
                    req.session_id
                );
                let _ = goal_manager.clear();
            }
            if let Err(e) = goal_manager.create(objective, criterion, None, None) {
                error!("Failed to create goal: {}", e);
            } else {
                info!("Goal created for session {}", req.session_id);
            }
        }
    }

    // Enqueue the user message
    {
        let msg = AiMessage::User(UserMessage::text(&req.prompt));
        sse_state.message_queue.lock().unwrap().enqueue(msg);
    }

    // Try to claim the in_flight flag
    let already_running = sse_state.in_flight.swap(true, Ordering::AcqRel);
    if already_running {
        debug!(
            "Message queued for session {} (agent running)",
            req.session_id
        );
        return (StatusCode::ACCEPTED, "Message queued").into_response();
    }

    // Store cancel_tx so /cancel can find it
    let (cancel_tx, _cancel_rx) = watch::channel(false);
    {
        let mut sessions = state.sse_sessions.write().await;
        if let Some(s) = sessions.get_mut(&req.session_id) {
            s.cancel_tx = Some(cancel_tx.clone());
        }
    }

    // Build shared closures (approve, question, on_event) that live across loop iterations
    let et = sse_state.event_tx.clone();
    let et_question = sse_state.event_tx.clone();
    let pa = sse_state.pending_approvals.clone();
    let pq = sse_state.pending_questions.clone();

    let approve: Option<pick_agent::core::state::ApproveFn> =
        Some(Arc::new(move |title: String, msg_body: String| {
            let pa = pa.clone();
            let et = et.clone();
            let approval_id = uuid::Uuid::now_v7().to_string();
            Box::pin(async move {
                let (tx, rx) = tokio::sync::oneshot::channel();
                pa.lock().unwrap().insert(approval_id.clone(), tx);
                let event = serde_json::json!({
                    "approval_id": approval_id,
                    "tool_name": title,
                    "tool_args": msg_body,
                    "source": "tool",
                });
                let _ = et.send(Ok(Event::default()
                    .event("approval_required")
                    .data(serde_json::to_string(&event).unwrap_or_default())));
                rx.await.unwrap_or(false)
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>
        }));

    let question: Option<pick_agent::core::state::QuestionFn> = Some(Arc::new(
        move |questions: Vec<pick_agent::core::state::QuestionPrompt>| {
            let pq = pq.clone();
            let et = et_question.clone();
            let question_id = uuid::Uuid::now_v7().to_string();
            Box::pin(async move {
                let (tx, rx) = tokio::sync::oneshot::channel();
                pq.lock().unwrap().insert(question_id.clone(), tx);
                let prompts: Vec<serde_json::Value> = questions
                    .iter()
                    .map(|q| {
                        serde_json::json!({
                            "question": q.question,
                            "header": q.header,
                            "options": q.options,
                            "multiple": q.multiple,
                        })
                    })
                    .collect();
                let payload = serde_json::json!({
                    "question_id": question_id,
                    "prompts": prompts,
                });
                let _ = et.send(Ok(Event::default()
                    .event("question")
                    .data(serde_json::to_string(&payload).unwrap_or_default())));
                rx.await.unwrap_or(Err("No response".to_string()))
            })
                as std::pin::Pin<
                    Box<dyn std::future::Future<Output = Result<Vec<Vec<String>>, String>> + Send>,
                >
        },
    ));

    let permission_manager = Arc::new(PermissionManager::new(
        "danger-full-access",
        &std::env::current_dir().unwrap_or_default(),
        None,
        &[],
    ));
    let sse_hook = Arc::new(SseApprovalHook {
        event_tx: sse_state.event_tx.clone(),
        pending_approvals: sse_state.pending_approvals.clone(),
    });
    permission_manager.register_permission_hook(sse_hook);

    let api_key = state
        .api_keys
        .read()
        .unwrap()
        .get(&session.provider)
        .cloned();
    let get_api_key = api_key.map(|key| {
        std::sync::Arc::new(move || Some(key.clone()))
            as std::sync::Arc<dyn Fn() -> Option<String> + Send + Sync>
    });

    let cwd = state.session_manager.get_cwd();
    let cwd_for_event = cwd.clone();
    let system_prompt = session.system_prompt.clone();
    let tools = {
        let gm = sse_state.goal_manager.read().unwrap().clone();
        pick_agent::tools::registry::create_coding_tools_with_goal_manager(
            Some(agent_mode.clone()),
            gm.unwrap_or_else(|| Arc::new(pick_agent::session::GoalManager::new())),
        )
    };

    let thinking_level = match req.thinking_level.as_deref() {
        Some("minimal") => ThinkingLevel::Minimal,
        Some("low") => ThinkingLevel::Low,
        Some("medium") => ThinkingLevel::Medium,
        Some("high") => ThinkingLevel::High,
        Some("xhigh") => ThinkingLevel::XHigh,
        _ => ThinkingLevel::Off,
    };

    let sid = req.session_id.clone();
    let state_agent = state.clone();

    tokio::spawn(async move {
        run_agent_loop_queue(
            state_agent,
            sid,
            sse_state,
            model,
            system_prompt,
            tools,
            thinking_level,
            approve,
            question,
            get_api_key,
            permission_manager,
            cwd,
            cwd_for_event,
            agent_mode,
        )
        .await;
    });

    debug!("Agent loop started for session {}", req.session_id);
    (StatusCode::ACCEPTED, "Agent started").into_response()
}

/// Start an AI code review for a session.
/// Unlike /ask, this uses a specialized review system prompt that defines the
/// reviewer role, tone guidance, pre-validation rules, and available tools.
#[utoipa::path(
    post,
    path = "/review/{session_id}",
    tag = "agent",
    responses(
        (status = 202, description = "Review started"),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn start_review(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let session = match state.session_manager.get(&session_id).await {
        Some(s) => s,
        None => {
            return (
                StatusCode::NOT_FOUND,
                format!("Session {} not found", session_id),
            )
                .into_response();
        }
    };

    let model = match pick_ai::models::get_model(&session.provider, &session.model_id) {
        Some(m) => m,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Model '{}' not found", session.model_id),
            )
                .into_response();
        }
    };

    let sse_state = {
        let sessions = state.sse_sessions.read().await;
        sessions.get(&session_id).cloned()
    };

    let sse_state = match sse_state {
        Some(s) => s,
        None => {
            return (
                StatusCode::PRECONDITION_FAILED,
                "No SSE connection for this session. Connect to /events/{session_id} first.",
            )
                .into_response();
        }
    };

    let agent_mode = sse_state.agent_mode.read().unwrap().clone();

    // Get workspace info for review context
    let cwd = session
        .cwd
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            state
                .config
                .cwd
                .as_ref()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
        });
    let git_info = tokio::task::spawn_blocking({
        let cwd = cwd.clone();
        move || get_git_info(&cwd)
    })
    .await
    .unwrap_or_else(|_| crate::git::GitInfo {
        branch: String::new(),
        changes: Vec::new(),
        cwd: String::new(),
    });

    let files_context: String = git_info
        .changes
        .iter()
        .map(|c| format!("  {} {}", c.status, c.path))
        .collect::<Vec<_>>()
        .join("\n");

    // Build the review system prompt
    let review_prompt = format!(
        "{}\n\n## Changes to Review\n\nCurrent branch: {}\nChanged files:\n{}\n\nReview these code changes.",
        REVIEW_SYSTEM_PROMPT,
        git_info.branch,
        if files_context.is_empty() {
            "  (no changes detected)"
        } else {
            &files_context
        },
    );

    // Enqueue a simple user message to trigger the review
    {
        let msg = AiMessage::User(UserMessage::text(
            "Please review the code changes in this workspace.",
        ));
        sse_state.message_queue.lock().unwrap().enqueue(msg);
    }

    // Try to claim the in_flight flag
    let already_running = sse_state.in_flight.swap(true, Ordering::AcqRel);
    if already_running {
        debug!("Review queued for session {} (agent running)", session_id);
        return (StatusCode::ACCEPTED, "Review queued").into_response();
    }

    // Store cancel_tx
    let (cancel_tx, _cancel_rx) = watch::channel(false);
    {
        let mut sessions = state.sse_sessions.write().await;
        if let Some(s) = sessions.get_mut(&session_id) {
            s.cancel_tx = Some(cancel_tx.clone());
        }
    }

    // Build shared closures (same as /ask)
    let et = sse_state.event_tx.clone();
    let et_question = sse_state.event_tx.clone();
    let pa = sse_state.pending_approvals.clone();
    let pq = sse_state.pending_questions.clone();

    let approve: Option<pick_agent::core::state::ApproveFn> =
        Some(Arc::new(move |title: String, msg_body: String| {
            let pa = pa.clone();
            let et = et.clone();
            let approval_id = uuid::Uuid::now_v7().to_string();
            Box::pin(async move {
                let (tx, rx) = tokio::sync::oneshot::channel();
                pa.lock().unwrap().insert(approval_id.clone(), tx);
                let event = serde_json::json!({
                    "approval_id": approval_id,
                    "tool_name": title,
                    "tool_args": msg_body,
                    "source": "tool",
                });
                let _ = et.send(Ok(Event::default()
                    .event("approval_required")
                    .data(serde_json::to_string(&event).unwrap_or_default())));
                rx.await.unwrap_or(false)
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>
        }));

    let question: Option<pick_agent::core::state::QuestionFn> = Some(Arc::new(
        move |questions: Vec<pick_agent::core::state::QuestionPrompt>| {
            let pq = pq.clone();
            let et = et_question.clone();
            let question_id = uuid::Uuid::now_v7().to_string();
            Box::pin(async move {
                let (tx, rx) = tokio::sync::oneshot::channel();
                pq.lock().unwrap().insert(question_id.clone(), tx);
                let prompts: Vec<serde_json::Value> = questions
                    .iter()
                    .map(|q| {
                        serde_json::json!({
                            "question": q.question,
                            "header": q.header,
                            "options": q.options,
                            "multiple": q.multiple,
                        })
                    })
                    .collect();
                let payload = serde_json::json!({
                    "question_id": question_id,
                    "prompts": prompts,
                });
                let _ = et.send(Ok(Event::default()
                    .event("question")
                    .data(serde_json::to_string(&payload).unwrap_or_default())));
                rx.await.unwrap_or(Err("No response".to_string()))
            })
                as std::pin::Pin<
                    Box<dyn std::future::Future<Output = Result<Vec<Vec<String>>, String>> + Send>,
                >
        },
    ));

    let permission_manager = Arc::new(PermissionManager::new(
        "danger-full-access",
        &std::env::current_dir().unwrap_or_default(),
        None,
        &[],
    ));
    let sse_hook = Arc::new(crate::approval::SseApprovalHook {
        event_tx: sse_state.event_tx.clone(),
        pending_approvals: sse_state.pending_approvals.clone(),
    });
    permission_manager.register_permission_hook(sse_hook);

    let api_key = state
        .api_keys
        .read()
        .unwrap()
        .get(&session.provider)
        .cloned();
    let get_api_key = api_key.map(|key| {
        std::sync::Arc::new(move || Some(key.clone()))
            as std::sync::Arc<dyn Fn() -> Option<String> + Send + Sync>
    });

    let cwd_for_event = cwd.clone();
    let tools = state.get_tools();

    let thinking_level = ThinkingLevel::Off;

    let sid = session_id.clone();
    let state_agent = state.clone();

    tokio::spawn(async move {
        run_agent_loop_queue(
            state_agent,
            sid,
            sse_state,
            model,
            review_prompt, // ← review-specific system prompt
            tools,
            thinking_level,
            approve,
            question,
            get_api_key,
            permission_manager,
            cwd,
            cwd_for_event,
            agent_mode,
        )
        .await;
    });

    debug!("Review started for session {}", session_id);
    (StatusCode::ACCEPTED, "Review started").into_response()
}

/// Sequential agent loop that drains the message queue between iterations.
/// This ensures messages are processed one after another without concurrent agent loops.
#[allow(clippy::too_many_arguments)]
async fn run_agent_loop_queue(
    state: Arc<AppState>,
    session_id: String,
    sse_state: SseSessionState,
    model: pick_ai::types::Model,
    system_prompt: String,
    tools: Vec<pick_agent::core::state::AgentTool>,
    thinking_level: ThinkingLevel,
    approve: Option<pick_agent::core::state::ApproveFn>,
    question: Option<pick_agent::core::state::QuestionFn>,
    get_api_key: Option<std::sync::Arc<dyn Fn() -> Option<String> + Send + Sync>>,
    permission_manager: Arc<PermissionManager>,
    cwd: std::path::PathBuf,
    cwd_for_event: std::path::PathBuf,
    agent_mode: String,
) {
    let et_on_event = sse_state.event_tx.clone();

    loop {
        // --- Drain queued messages ---
        let queued_msgs = { sse_state.message_queue.lock().unwrap().drain_all() };

        // Emit message_dequeued for each message picked up from the queue
        for msg in &queued_msgs {
            if let AiMessage::User(user_msg) = msg {
                let text: String = user_msg
                    .content
                    .iter()
                    .filter_map(|b| {
                        if let pick_ai::types::ContentBlock::Text(t) = b {
                            Some(t.text.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                if !text.is_empty() {
                    let _ = et_on_event.send(Ok(Event::default()
                        .event("message_dequeued")
                        .data(serde_json::json!({"text": text}).to_string())));
                }
            }
        }

        // --- Get session messages ---
        let session = state.session_manager.get(&session_id).await;
        let mut all_msgs = session
            .as_ref()
            .map(|s| s.messages.clone())
            .unwrap_or_default();
        all_msgs.extend(queued_msgs);

        if all_msgs.is_empty() {
            // Nothing to process — mark done and exit
            cleanup_loop(&state, &session_id, &sse_state, false).await;
            break;
        }

        // --- Create per-iteration cancel_tx ---
        let (cancel_tx, cancel_rx) = watch::channel(false);
        {
            let mut sessions = state.sse_sessions.write().await;
            if let Some(s) = sessions.get_mut(&session_id) {
                s.cancel_tx = Some(cancel_tx.clone());
            }
        }

        let mq_steer = sse_state.message_queue.clone();
        let mq_follow = sse_state.message_queue.clone();
        let et_for_event = sse_state.event_tx.clone();
        let et_steer = sse_state.event_tx.clone();
        let et_follow = sse_state.event_tx.clone();
        let et_for_git = et_on_event.clone();
        let cwd_for_git = cwd_for_event.clone();
        let gm_steer = sse_state.goal_manager.clone();
        let gm_follow = sse_state.goal_manager.clone();

        // Build mode-specific ruleset
        let ruleset = match agent_mode.as_str() {
            "plan" => Ruleset::new(vec![
                Rule::new("read", "*", Action::Allow),
                Rule::new("grep", "*", Action::Allow),
                Rule::new("glob", "*", Action::Allow),
                Rule::new("list", "*", Action::Allow),
                Rule::new("question", "*", Action::Allow),
                Rule::new("subagent", "*", Action::Allow),
                Rule::new("edit", "*", Action::Deny),
                Rule::new("edit", ".pick/plans/*.md", Action::Allow),
                Rule::new("bash", "ls", Action::Allow),
                Rule::new("bash", "cat", Action::Allow),
                Rule::new("bash", "head", Action::Allow),
                Rule::new("bash", "tail", Action::Allow),
                Rule::new("bash", "rg", Action::Allow),
                Rule::new("bash", "grep", Action::Allow),
                Rule::new("bash", "find", Action::Allow),
                Rule::new("bash", "which", Action::Allow),
                Rule::new("bash", "stat", Action::Allow),
                Rule::new("bash", "wc", Action::Allow),
                Rule::new("bash", "diff", Action::Allow),
                Rule::new("bash", "sort", Action::Allow),
                Rule::new("bash", "uniq", Action::Allow),
                Rule::new("bash", "echo", Action::Allow),
                Rule::new("bash", "pwd", Action::Allow),
                Rule::new("bash", "type", Action::Allow),
                Rule::new("bash", "where", Action::Allow),
                Rule::new("bash", "dir", Action::Allow),
                Rule::new("bash", "more", Action::Allow),
                Rule::new("bash", "less", Action::Allow),
                Rule::new("bash", "printf", Action::Allow),
                Rule::new("bash", "env", Action::Allow),
                Rule::new("bash", "printenv", Action::Allow),
                Rule::new("bash", "git diff", Action::Allow),
                Rule::new("bash", "git log", Action::Allow),
                Rule::new("bash", "git show", Action::Allow),
                Rule::new("bash", "git status", Action::Allow),
                Rule::new("bash", "git branch", Action::Allow),
                Rule::new("bash", "git ls-files", Action::Allow),
                Rule::new("bash", "git rev-parse", Action::Allow),
                Rule::new("bash", "git rev-list", Action::Allow),
                Rule::new("bash", "git describe", Action::Allow),
                Rule::new("bash", "git config", Action::Allow),
                Rule::new("bash", "*", Action::Deny),
                Rule::new("plan_enter", "*", Action::Deny),
                Rule::new("plan_exit", "*", Action::Allow),
            ]),
            _ => Ruleset::new(vec![
                Rule::new("read", "*", Action::Allow),
                Rule::new("grep", "*", Action::Allow),
                Rule::new("glob", "*", Action::Allow),
                Rule::new("list", "*", Action::Allow),
                Rule::new("question", "*", Action::Allow),
                Rule::new("subagent", "*", Action::Allow),
                Rule::new("edit", "*", Action::Allow),
                Rule::new("bash", "*", Action::Allow),
                Rule::new("webfetch", "*", Action::Allow),
                Rule::new("plan_enter", "*", Action::Allow),
                Rule::new("plan_exit", "*", Action::Deny),
            ]),
        };

        let fs_policy = Arc::new(FileSystemPolicy::new_workspace_default(&cwd));
        let agent_registry = AgentRegistry::new();

        let tool_execution_permission = pick_agent::settings::SettingsManager::load_from_paths(
            pick_agent::settings::get_global_settings_path(),
            pick_agent::settings::get_project_settings_path(&cwd),
        )
        .get()
        .tool_execution_permission
        .clone();

        let config = AgentLoopConfig {
            model: model.clone(),
            system_prompt: system_prompt.clone(),
            developer_sections: vec![],
            tools: tools.clone(),
            thinking_level,
            max_tokens: None,
            temperature: None,
            extension_runner: None,
            transform_context: None,
            get_api_key: get_api_key.clone(),
            before_tool_call: None,
            should_stop_after_turn: None,
            get_steering_messages: Some(Arc::new(move || {
                let mut msgs = mq_steer.lock().unwrap().drain();
                for msg in &msgs {
                    if let AiMessage::User(user_msg) = msg {
                        let text: String = user_msg
                            .content
                            .iter()
                            .filter_map(|b| {
                                if let pick_ai::types::ContentBlock::Text(t) = b {
                                    Some(t.text.clone())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if !text.is_empty() {
                            let _ = et_steer.send(Ok(Event::default()
                                .event("message_dequeued")
                                .data(serde_json::json!({"text": text}).to_string())));
                        }
                    }
                }
                if let Some(goal_manager) = gm_steer.read().unwrap().as_ref() {
                    if let Some(goal) = goal_manager.get()
                        && goal.status == "active"
                    {
                        let msg_text =
                            pick_agent::templates::render_steering_active(&goal, goal_manager);
                        msgs.push(AiMessage::User(UserMessage::text(msg_text)));
                    }
                }
                msgs
            })),
            get_follow_up_messages: Some(Arc::new(move |_result| {
                let mut msgs = mq_follow.lock().unwrap().drain();
                for msg in &msgs {
                    if let AiMessage::User(user_msg) = msg {
                        let text: String = user_msg
                            .content
                            .iter()
                            .filter_map(|b| {
                                if let pick_ai::types::ContentBlock::Text(t) = b {
                                    Some(t.text.clone())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if !text.is_empty() {
                            let _ = et_follow.send(Ok(Event::default()
                                .event("message_dequeued")
                                .data(serde_json::json!({"text": text}).to_string())));
                        }
                    }
                }
                if msgs.is_empty() {
                    if let Some(goal_manager) = gm_follow.read().unwrap().as_ref() {
                        if goal_manager.can_continue() {
                            if let Some(goal) = goal_manager.get()
                                && goal.status == "active"
                            {
                                let _ = goal_manager.register_continuation();
                                let msg_text = pick_agent::templates::render_follow_up_continuation(
                                    &goal,
                                    goal_manager,
                                );
                                msgs.push(AiMessage::User(UserMessage::text(msg_text)));
                            }
                        }
                    }
                }
                msgs
            })),
            provider_max_retries: None,
            provider_max_retry_delay_ms: None,
            approve: approve.clone(),
            question: question.clone(),
            agent_id: None,
            agent_registry: Some(agent_registry),
            on_event: Some(Arc::new(move |event| {
                for server_event in events::serialize_event(&event) {
                    let sse_event = Event::default()
                        .event(&server_event.event_type)
                        .data(serde_json::to_string(&server_event.payload).unwrap_or_default());
                    let _ = et_for_event.send(Ok(sse_event));
                }
                if matches!(event, pick_agent::core::events::AgentEvent::TurnEnd { .. }) {
                    let et = et_for_git.clone();
                    let cwd = cwd_for_git.clone();
                    tokio::spawn(async move {
                        let git_info = get_git_info(&cwd);
                        let payload = serde_json::to_value(&git_info).unwrap_or_default();
                        let sse_event = Event::default()
                            .event("git_info_updated")
                            .data(serde_json::to_string(&payload).unwrap_or_default());
                        let _ = et.send(Ok(sse_event));
                    });
                }
            })),
            fs_policy: Some(fs_policy.clone()),
            cwd: Some(cwd.clone()),
            mode_rulesets: Some(vec![ruleset]),
            permission_hooks: Some(permission_manager.hook_registry.clone()),
            permission_manager: Some(permission_manager.clone()),
            tool_event_bus: None,
            sandbox: None,
            sandbox_enabled: None,
            cancel_signal_tx: Some(Arc::new(cancel_tx)),
            skill_paths: Vec::new(),
            parent_goal_manager: sse_state.goal_manager.read().unwrap().clone(),
            on_turn_complete: None,
            tool_execution_permission,
        };

        let et_agent = sse_state.event_tx.clone();
        let sid = session_id.clone();

        let result = pick_agent::core::agent_loop::run_agent_loop(config, all_msgs).await;

        match result {
            Ok(agent_result) => {
                let total_input: u64 = agent_result
                    .messages
                    .iter()
                    .filter_map(|m| {
                        if let AiMessage::Assistant(a) = m {
                            Some(a.usage.input)
                        } else {
                            None
                        }
                    })
                    .sum();
                let total_output: u64 = agent_result
                    .messages
                    .iter()
                    .filter_map(|m| {
                        if let AiMessage::Assistant(a) = m {
                            Some(a.usage.output)
                        } else {
                            None
                        }
                    })
                    .sum();

                state
                    .session_manager
                    .update_messages(&sid, agent_result.messages)
                    .await;

                // Auto-generate session title from first user message
                let generated_title = {
                    let session = state.session_manager.get(&sid).await;
                    if let Some(session) = session {
                        let is_default = session.title.starts_with("New session -")
                            || session.title.starts_with("Session -");
                        if is_default
                            && let Some(first_text) =
                                pick_agent::session::title::first_user_text(&session.messages)
                            && let Some(model) =
                                pick_ai::models::get_model(&session.provider, &session.model_id)
                        {
                            let api_key = state
                                .api_keys
                                .read()
                                .unwrap()
                                .get(&session.provider)
                                .cloned();
                            let title = pick_agent::session::title::generate_title(
                                &first_text,
                                &model,
                                api_key,
                            )
                            .await;
                            if let Some(t) = title {
                                state
                                    .session_manager
                                    .update_session(&sid, Some(t.clone()), None, None, None, None)
                                    .await;
                                Some(t)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };

                let agent_end_event =
                    events::serialize_agent_end(total_input, total_output, generated_title);
                let sse_event = Event::default()
                    .event(&agent_end_event.event_type)
                    .data(serde_json::to_string(&agent_end_event.payload).unwrap_or_default());
                let _ = et_agent.send(Ok(sse_event));

                // Send goal_updated event so the frontend can update the status bar
                let goal_payload = {
                    let guard = sse_state.goal_manager.read().unwrap();
                    guard
                        .as_ref()
                        .and_then(|gm| gm.get())
                        .and_then(|entry| serde_json::to_value(&entry).ok())
                };
                if let Some(payload) = goal_payload {
                    let sse_event = Event::default()
                        .event("goal_updated")
                        .data(serde_json::to_string(&payload).unwrap_or_default());
                    let _ = et_agent.send(Ok(sse_event));
                }

                // Check if cancelled by user
                if *cancel_rx.borrow() {
                    debug!("Agent loop cancelled by user for session {}", sid);
                    cleanup_loop(&state, &session_id, &sse_state, true).await;
                    break;
                }

                // If no more messages queued, stop looping
                if sse_state.message_queue.lock().unwrap().is_empty() {
                    cleanup_loop(&state, &session_id, &sse_state, false).await;
                    break;
                }
                // Otherwise continue loop to process next queued messages
            }
            Err(e) => {
                error!("Agent loop error for session {}: {}", session_id, e);
                let sse_event = Event::default().event("error").data(
                    serde_json::to_string(&serde_json::json!({"message": e})).unwrap_or_default(),
                );
                let _ = et_agent.send(Ok(sse_event));
                cleanup_loop(&state, &session_id, &sse_state, true).await;
                break;
            }
        }
    }
}

/// Clean up after an agent loop iteration: clear cancel_tx, clear queue on cancel, mark in_flight false.
async fn cleanup_loop(
    state: &Arc<AppState>,
    session_id: &str,
    sse_state: &SseSessionState,
    clear_queue: bool,
) {
    if clear_queue {
        sse_state.message_queue.lock().unwrap().clear();
    }
    sse_state.in_flight.store(false, Ordering::Release);
    let mut sessions = state.sse_sessions.write().await;
    if let Some(s) = sessions.get_mut(session_id) {
        s.cancel_tx = None;
    }
}

/// Cancel an active agent and clear any queued messages
#[utoipa::path(
    post,
    path = "/cancel",
    tag = "agent",
    request_body = CancelRequest,
    responses(
        (status = 200, description = "Agent cancelled"),
        (status = 404, description = "No active agent for this session"),
    )
)]
pub async fn cancel(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CancelRequest>,
) -> impl IntoResponse {
    let sessions = state.sse_sessions.read().await;
    if let Some(s) = sessions.get(&req.session_id) {
        // Clear pending message queue
        s.message_queue.lock().unwrap().clear();
        // Cancel running agent loop if any
        if let Some(ref tx) = s.cancel_tx {
            let _ = tx.send(true);
            return (StatusCode::OK, "Cancelled").into_response();
        }
        // No running agent but queue cleared
        return (StatusCode::OK, "Cancelled").into_response();
    }
    (StatusCode::NOT_FOUND, "No active agent for this session").into_response()
}

/// Approve or deny a pending tool approval
#[utoipa::path(
    post,
    path = "/approve",
    tag = "agent",
    request_body = ApproveRequest,
    responses(
        (status = 200, description = "Approval responded"),
        (status = 404, description = "Approval request not found"),
    )
)]
pub async fn approve(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ApproveRequest>,
) -> impl IntoResponse {
    let sessions = state.sse_sessions.read().await;
    if let Some(s) = sessions.get(&req.session_id) {
        let mut map = s.pending_approvals.lock().unwrap();
        if let Some(tx) = map.remove(&req.approval_id) {
            let _ = tx.send(req.approved);
            return (StatusCode::OK, "Approved").into_response();
        }
    }
    (StatusCode::NOT_FOUND, "Approval request not found").into_response()
}

/// Answer a pending agent question
#[utoipa::path(
    post,
    path = "/answer_question",
    tag = "agent",
    request_body = AnswerQuestionRequest,
    responses(
        (status = 200, description = "Question answered"),
        (status = 404, description = "Question not found"),
    )
)]
pub async fn answer_question(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AnswerQuestionRequest>,
) -> impl IntoResponse {
    let sessions = state.sse_sessions.read().await;
    if let Some(s) = sessions.get(&req.session_id) {
        let mut map = s.pending_questions.lock().unwrap();
        if let Some(tx) = map.remove(&req.question_id) {
            let _ = tx.send(Ok(req.answers));
            return (StatusCode::OK, "Answered").into_response();
        }
    }
    (StatusCode::NOT_FOUND, "Question not found").into_response()
}
