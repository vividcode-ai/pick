use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::Event;
use pick_agent::core::agent_loop::AgentLoopConfig;
use pick_agent::core::state::ThinkingLevel;
use pick_agent::permission::manager::PermissionManager;
use pick_ai::types::Message as AiMessage;
use pick_ai::types::UserMessage;
use serde::Deserialize;
use tokio::sync::watch;
use tracing::{error, info};
use utoipa::ToSchema;

use crate::AppState;
use crate::approval::SseApprovalHook;
use crate::events;
use crate::git::get_git_info;
use crate::session::SseSessionState;

#[derive(Deserialize, ToSchema)]
pub struct AskRequest {
    pub session_id: String,
    pub prompt: String,
    pub thinking_level: Option<String>,
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

    // Enqueue the user message
    {
        let msg = AiMessage::User(UserMessage::text(&req.prompt));
        sse_state.message_queue.lock().unwrap().enqueue(msg);
    }

    // Try to claim the in_flight flag
    let already_running = sse_state.in_flight.swap(true, Ordering::AcqRel);
    if already_running {
        info!(
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

    let api_key = state.api_keys.get(&session.provider).cloned();
    let get_api_key = api_key.map(|key| {
        std::sync::Arc::new(move || Some(key.clone()))
            as std::sync::Arc<dyn Fn() -> Option<String> + Send + Sync>
    });

    let cwd = state.session_manager.get_cwd();
    let cwd_for_event = cwd.clone();
    let system_prompt = session.system_prompt.clone();
    let tools = session.tools.clone();

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
        )
        .await;
    });

    info!("Agent loop started for session {}", req.session_id);
    (StatusCode::ACCEPTED, "Agent started").into_response()
}

/// Sequential agent loop that drains the message queue between iterations.
/// This ensures messages are processed one after another without concurrent agent loops.
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
) {
    let et_on_event = sse_state.event_tx.clone();

    loop {
        // --- Drain queued messages ---
        let queued_msgs = { sse_state.message_queue.lock().unwrap().drain_all() };

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
        let et_for_git = et_on_event.clone();
        let cwd_for_git = cwd_for_event.clone();

        let config = AgentLoopConfig {
            model: model.clone(),
            system_prompt: system_prompt.clone(),
            tools: tools.clone(),
            thinking_level: thinking_level.clone(),
            max_tokens: None,
            temperature: None,
            extension_runner: None,
            transform_context: None,
            get_api_key: get_api_key.clone(),
            before_tool_call: None,
            should_stop_after_turn: None,
            get_steering_messages: Some(Arc::new(move || mq_steer.lock().unwrap().drain())),
            get_follow_up_messages: Some(Arc::new(move |_result| {
                mq_follow.lock().unwrap().drain()
            })),
            provider_max_retries: None,
            provider_max_retry_delay_ms: None,
            approve: approve.clone(),
            question: question.clone(),
            agent_id: None,
            agent_registry: None,
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
            fs_policy: None,
            cwd: Some(cwd.clone()),
            mode_rulesets: None,
            permission_hooks: Some(permission_manager.hook_registry.clone()),
            permission_manager: Some(permission_manager.clone()),
            tool_event_bus: None,
            sandbox: None,
            sandbox_enabled: None,
            cancel_signal_tx: Some(Arc::new(cancel_tx)),
            skill_paths: Vec::new(),
            on_turn_complete: None,
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
                            let api_key = state.api_keys.get(&session.provider).cloned();
                            let title = pick_agent::session::title::generate_title(
                                &first_text,
                                &model,
                                api_key,
                            )
                            .await;
                            if let Some(t) = title {
                                state
                                    .session_manager
                                    .update_session(&sid, Some(t.clone()), None, None)
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

                // Check if cancelled by user
                if *cancel_rx.borrow() {
                    info!("Agent loop cancelled by user for session {}", sid);
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
