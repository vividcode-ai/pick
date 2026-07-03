use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::AppState;
use crate::git::get_git_info;
use crate::session::SessionInfo;

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct CreateSessionRequest {
    pub model_id: Option<String>,
    pub provider: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct CreateSessionResponse {
    pub session_id: String,
    pub title: String,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateSessionRequest {
    pub title: Option<String>,
    pub model_id: Option<String>,
    pub provider: Option<String>,
    pub archived: Option<bool>,
}

#[derive(Deserialize)]
pub struct MessagesQuery {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Deserialize)]
pub struct ForkQuery {
    pub message_count: Option<usize>,
}

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Server is healthy", body = HealthResponse),
    )
)]
pub async fn health() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// List all sessions with rich info
#[utoipa::path(
    get,
    path = "/sessions",
    tag = "sessions",
    responses(
        (status = 200, description = "List of sessions", body = Vec<SessionInfo>),
    )
)]
pub async fn list_sessions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let sessions = state.session_manager.list_info().await;
    Json(sessions)
}

/// Create a new session
#[utoipa::path(
    post,
    path = "/sessions",
    tag = "sessions",
    request_body = CreateSessionRequest,
    responses(
        (status = 201, description = "Session created", body = CreateSessionResponse),
    )
)]
pub async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    let provider = req.provider.unwrap_or_else(|| "anthropic".to_string());
    let model_id = req
        .model_id
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
    let system_prompt = state.build_system_prompt(&provider, &model_id);
    let tools = state.get_tools();

    let (session_id, title) = state
        .session_manager
        .create(model_id, provider, system_prompt, tools)
        .await;

    (
        StatusCode::CREATED,
        Json(CreateSessionResponse { session_id, title }),
    )
}

/// Get session details
#[utoipa::path(
    get,
    path = "/sessions/{id}",
    tag = "sessions",
    params(
        ("id" = String, Path, description = "Session ID"),
    ),
    responses(
        (status = 200, description = "Session details"),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn get_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<SessionInfo>, StatusCode> {
    let session = state.session_manager.get(&id).await;
    match session {
        Some(s) => Ok(Json(s.to_info())),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Delete a session
#[utoipa::path(
    delete,
    path = "/sessions/{id}",
    tag = "sessions",
    params(
        ("id" = String, Path, description = "Session ID"),
    ),
    responses(
        (status = 204, description = "Session deleted"),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn delete_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> StatusCode {
    if state.session_manager.delete(&id).await {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

/// Update session metadata
#[utoipa::path(
    patch,
    path = "/sessions/{id}",
    tag = "sessions",
    params(
        ("id" = String, Path, description = "Session ID"),
    ),
    request_body = UpdateSessionRequest,
    responses(
        (status = 200, description = "Session updated"),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn update_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateSessionRequest>,
) -> StatusCode {
    if state
        .session_manager
        .update_session(&id, req.title, req.model_id, req.provider, req.archived)
        .await
    {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

/// Fork a session
#[utoipa::path(
    post,
    path = "/sessions/{id}/fork",
    tag = "sessions",
    params(
        ("id" = String, Path, description = "Session ID to fork from"),
    ),
    responses(
        (status = 201, description = "Session forked", body = CreateSessionResponse),
        (status = 404, description = "Source session not found"),
    )
)]
pub async fn fork_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<ForkQuery>,
) -> impl IntoResponse {
    match state.session_manager.fork(&id, query.message_count).await {
        Some((new_id, title)) => (
            StatusCode::CREATED,
            Json(CreateSessionResponse {
                session_id: new_id,
                title,
            }),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND).into_response(),
    }
}

/// Get session messages with pagination
#[utoipa::path(
    get,
    path = "/sessions/{id}/messages",
    tag = "sessions",
    params(
        ("id" = String, Path, description = "Session ID"),
        ("offset" = Option<usize>, Query, description = "Message offset"),
        ("limit" = Option<usize>, Query, description = "Max messages"),
    ),
    responses(
        (status = 200, description = "Session messages"),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn get_session_messages(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<MessagesQuery>,
) -> impl IntoResponse {
    match state
        .session_manager
        .get_messages(&id, query.offset, query.limit)
        .await
    {
        Some((messages, total)) => Json(serde_json::json!({
            "messages": messages,
            "total": total,
            "offset": query.offset.unwrap_or(0),
        }))
        .into_response(),
        None => (StatusCode::NOT_FOUND).into_response(),
    }
}

/// Get session status
#[utoipa::path(
    get,
    path = "/sessions/{id}/status",
    tag = "sessions",
    params(
        ("id" = String, Path, description = "Session ID"),
    ),
    responses(
        (status = 200, description = "Session status"),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn get_session_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.session_manager.get(&id).await {
        Some(s) => Json(serde_json::json!({
            "id": s.id,
            "status": s.status,
            "message_count": s.messages.len(),
            "updated_at": s.updated_at,
        }))
        .into_response(),
        None => (StatusCode::NOT_FOUND).into_response(),
    }
}

#[derive(Serialize, ToSchema)]
pub struct SummarizeResponse {
    pub summary: String,
}

fn extract_text(content: &[pick_ai::ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|b| match b {
            pick_ai::ContentBlock::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Summarize a session's conversation
#[utoipa::path(
    post,
    path = "/sessions/{id}/summarize",
    tag = "sessions",
    params(
        ("id" = String, Path, description = "Session ID"),
    ),
    responses(
        (status = 200, description = "Session summarized", body = SummarizeResponse),
        (status = 404, description = "Session not found"),
        (status = 500, description = "Summarization failed"),
    )
)]
pub async fn summarize_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let session = match state.session_manager.get(&id).await {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, "Session not found").into_response(),
    };

    let model = match pick_ai::models::get_model(&session.provider, &session.model_id) {
        Some(m) => m,
        None => return (StatusCode::BAD_REQUEST, "Model not found").into_response(),
    };

    let conversation: String = session
        .messages
        .iter()
        .map(|m| match m {
            pick_ai::Message::User(u) => format!("User: {}", extract_text(&u.content)),
            pick_ai::Message::Assistant(a) => format!("Assistant: {}", extract_text(&a.content)),
            pick_ai::Message::ToolResult(t) => {
                format!(
                    "Tool ({}) output:\n{}",
                    t.tool_name,
                    extract_text(&t.content)
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let prompt = format!(
        "Please summarize the following conversation concisently, highlighting key problems, solutions, and decisions:\n\n{}",
        conversation
    );

    let api_key = state.api_keys.get(&session.provider).cloned();

    let context = pick_ai::Context {
        system_prompt: Some("You are a concise summarizer.".into()),
        messages: vec![pick_ai::Message::User(pick_ai::UserMessage::text(&prompt))],
        tools: None,
    };

    let result =
        pick_ai::complete_simple(&model, context, api_key, None, Some(1000), None, None).await;

    if let Some(err) = &result.error_message {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("AI error: {err}"),
        )
            .into_response();
    }

    let summary = extract_text(&result.content);

    (StatusCode::OK, Json(SummarizeResponse { summary })).into_response()
}

#[derive(Serialize, ToSchema)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub reasoning: bool,
}

#[derive(Serialize, ToSchema)]
pub struct ProviderInfo {
    pub provider: String,
    pub has_key: bool,
    pub models: Vec<ModelInfo>,
}

/// List all configured AI providers and their models
#[utoipa::path(
    get,
    path = "/providers",
    tag = "providers",
    responses(
        (status = 200, description = "List of providers with models", body = Vec<ProviderInfo>),
    )
)]
pub async fn list_providers(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let providers: Vec<ProviderInfo> = pick_ai::models::get_providers()
        .iter()
        .map(|p| {
            let has_key = state.api_keys.get(p).is_some_and(|k| !k.is_empty());
            let models = pick_ai::models::get_models(p)
                .iter()
                .map(|m| ModelInfo {
                    id: m.id.clone(),
                    name: m.name.clone(),
                    reasoning: m.reasoning,
                })
                .collect();
            ProviderInfo {
                provider: p.clone(),
                has_key,
                models,
            }
        })
        .collect();
    Json(providers)
}

/// Get git info for a session's workspace
#[utoipa::path(
    get,
    path = "/sessions/{id}/git-info",
    tag = "sessions",
    params(
        ("id" = String, Path, description = "Session ID"),
    ),
    responses(
        (status = 200, description = "Git info"),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn get_session_git_info(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let session = match state.session_manager.get(&id).await {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, "Session not found").into_response(),
    };
    let cwd = match session.cwd {
        Some(ref c) => std::path::PathBuf::from(c),
        None => return (StatusCode::NOT_FOUND, "No workspace directory").into_response(),
    };
    let git_info = get_git_info(&cwd);
    Json(git_info).into_response()
}

#[derive(Serialize, ToSchema)]
pub struct ServerConfigResponse {
    pub host: String,
    pub port: u16,
    pub pty_ws_port: u16,
}

pub async fn server_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(ServerConfigResponse {
        host: state.config.host.clone(),
        port: state.config.port,
        pty_ws_port: state.config.pty_ws_port,
    })
}
