use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::Event;
use pick_agent::core::agent_loop::AgentLoopConfig;
use pick_agent::core::state::ThinkingLevel;
use pick_ai::types::Message as AiMessage;
use pick_ai::types::UserMessage;
use serde::Deserialize;
use tokio::sync::watch;
use tracing::{error, info};
use utoipa::ToSchema;

use crate::AppState;
use crate::events;
use crate::git::get_git_info;

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

/// Submit a prompt to an agent session
#[utoipa::path(
    post,
    path = "/ask",
    tag = "agent",
    request_body = AskRequest,
    responses(
        (status = 202, description = "Agent started"),
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

    let msg = AiMessage::User(UserMessage::text(&req.prompt));
    let all_msgs = {
        let mut msgs = session.messages.clone();
        msgs.push(msg);
        msgs
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

    let (cancel_tx, _cancel_rx) = watch::channel(false);

    // Store cancel_tx in the SSE session state
    {
        let mut sessions = state.sse_sessions.write().await;
        if let Some(s) = sessions.get_mut(&req.session_id) {
            s.cancel_tx = Some(cancel_tx.clone());
        }
    }

    let et = sse_state.event_tx.clone();
    let et_question = sse_state.event_tx.clone();
    let et_on_event = sse_state.event_tx.clone();
    let pa = sse_state.pending_approvals.clone();
    let pq = sse_state.pending_questions.clone();

    let approve = Some(Arc::new(move |title: String, msg_body: String| {
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
            });
            let _ = et.send(Ok(Event::default()
                .event("approval_required")
                .data(serde_json::to_string(&event).unwrap_or_default())));
            rx.await.unwrap_or(false)
        }) as std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>
    }) as pick_agent::core::state::ApproveFn);

    let question = Some(Arc::new(
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
    ) as pick_agent::core::state::QuestionFn);

    let api_key = state.api_keys.get(&session.provider).cloned();
    let get_api_key = api_key.map(|key| {
        std::sync::Arc::new(move || Some(key.clone()))
            as std::sync::Arc<dyn Fn() -> Option<String> + Send + Sync>
    });

    let cwd = state.session_manager.get_cwd();
    let cwd_for_event = cwd.clone();
    let et_for_git = et_on_event.clone();

    let config = AgentLoopConfig {
        model: model.clone(),
        system_prompt: session.system_prompt.clone(),
        tools: session.tools.clone(),
        thinking_level: match req.thinking_level.as_deref() {
            Some("minimal") => ThinkingLevel::Minimal,
            Some("low") => ThinkingLevel::Low,
            Some("medium") => ThinkingLevel::Medium,
            Some("high") => ThinkingLevel::High,
            Some("xhigh") => ThinkingLevel::XHigh,
            _ => ThinkingLevel::Off,
        },
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
        approve,
        question,
        agent_id: None,
        agent_registry: None,
        on_event: Some(Arc::new(move |event| {
            for server_event in events::serialize_event(&event) {
                let sse_event = Event::default()
                    .event(&server_event.event_type)
                    .data(serde_json::to_string(&server_event.payload).unwrap_or_default());
                let _ = et_on_event.send(Ok(sse_event));
            }
            if matches!(event, pick_agent::core::events::AgentEvent::TurnEnd { .. }) {
                let et = et_for_git.clone();
                let cwd = cwd_for_event.clone();
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
        cwd: Some(cwd),
        mode_rulesets: None,
        permission_hooks: None,
        permission_manager: None,
        tool_event_bus: None,
        sandbox: None,
        sandbox_enabled: None,
        cancel_signal_tx: Some(Arc::new(cancel_tx)),
        skill_paths: Vec::new(),
        on_turn_complete: None,
    };

    let et_agent = sse_state.event_tx.clone();
    let sid_agent = req.session_id.clone();
    let state_agent = state.clone();

    tokio::spawn(async move {
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

                state_agent
                    .session_manager
                    .update_messages(&sid_agent, agent_result.messages)
                    .await;

                // Auto-generate session title from first user message
                let generated_title = {
                    let session = state_agent.session_manager.get(&sid_agent).await;
                    if let Some(session) = session {
                        let is_default = session.title.starts_with("New session -")
                            || session.title.starts_with("Session -");
                        if is_default
                            && let Some(first_text) =
                                pick_agent::session::title::first_user_text(&session.messages)
                            && let Some(model) =
                                pick_ai::models::get_model(&session.provider, &session.model_id)
                        {
                            let api_key = state_agent.api_keys.get(&session.provider).cloned();
                            let title = pick_agent::session::title::generate_title(
                                &first_text,
                                &model,
                                api_key,
                            )
                            .await;
                            if let Some(t) = title {
                                state_agent
                                    .session_manager
                                    .update_session(&sid_agent, Some(t.clone()), None, None)
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
            }
            Err(e) => {
                error!("Agent loop error for session {}: {}", sid_agent, e);
                let sse_event = Event::default().event("error").data(
                    serde_json::to_string(&serde_json::json!({"message": e})).unwrap_or_default(),
                );
                let _ = et_agent.send(Ok(sse_event));
            }
        }
    });

    info!("Agent spawned for session {}", req.session_id);
    (StatusCode::ACCEPTED, "Agent started").into_response()
}

/// Cancel an active agent
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
    if let Some(s) = sessions.get(&req.session_id)
        && let Some(ref tx) = s.cancel_tx
    {
        let _ = tx.send(true);
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
