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
use crate::git::{
    GitDiffEntry, GitDiffsResponse, get_git_diffs, get_git_info, get_git_single_diff,
    list_git_branches,
};
use crate::session::SessionInfo;
use pick_agent::auth as auth_storage;

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct CreateSessionRequest {
    pub model_id: Option<String>,
    pub provider: Option<String>,
    pub thinking_level: Option<String>,
    /// Project working directory to associate this session with.
    /// When absent the current server cwd is used.
    pub cwd: Option<String>,
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
    pub thinking_level: Option<String>,
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
    let sessions = state.session_manager.read().await.list_info().await;
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
    let thinking_level = req.thinking_level.unwrap_or_else(|| "off".to_string());
    let system_prompt = state.build_system_prompt(&provider, &model_id);
    let tools = state.get_tools();

    let (session_id, title) = state
        .session_manager
        .write()
        .await
        .create(
            model_id,
            provider,
            thinking_level,
            system_prompt,
            tools,
            req.cwd,
        )
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
    let session = state.session_manager.read().await.get(&id).await;
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
    if state.session_manager.write().await.delete(&id).await {
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
        .write()
        .await
        .update_session(
            &id,
            req.title,
            req.model_id,
            req.provider,
            req.thinking_level,
            req.archived,
        )
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
    match state
        .session_manager
        .write()
        .await
        .fork(&id, query.message_count)
        .await
    {
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
        .read()
        .await
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
    match state.session_manager.read().await.get(&id).await {
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
    let session = match state.session_manager.read().await.get(&id).await {
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

    let api_key = state
        .api_keys
        .read()
        .unwrap()
        .get(&session.provider)
        .cloned();

    let context = pick_ai::Context {
        system_prompt: Some("You are a concise summarizer.".into()),
        developer_messages: vec![],
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

#[derive(Serialize, ToSchema)]
pub struct ProvidersResponse {
    pub providers: Vec<ProviderInfo>,
    pub last_provider: Option<String>,
    pub last_model: Option<String>,
    pub thinking_level: Option<String>,
}

/// List all configured AI providers and their models
#[utoipa::path(
    get,
    path = "/providers",
    tag = "providers",
    responses(
        (status = 200, description = "List of providers with models", body = ProvidersResponse),
    )
)]
pub async fn list_providers(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let providers: Vec<ProviderInfo> = pick_ai::models::get_providers()
        .iter()
        .map(|p| {
            let has_key = state
                .api_keys
                .read()
                .unwrap()
                .get(p)
                .is_some_and(|k| !k.is_empty());
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
    let last_provider = state.last_provider.read().unwrap().clone();
    let last_model = state.last_model.read().unwrap().clone();
    let thinking_level = state.thinking_level.read().unwrap().clone();
    Json(ProvidersResponse {
        providers,
        last_provider,
        last_model,
        thinking_level,
    })
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
    let session = match state.session_manager.read().await.get(&id).await {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, "Session not found").into_response(),
    };
    let cwd = match session.cwd {
        Some(ref c) => std::path::PathBuf::from(c),
        None => return (StatusCode::NOT_FOUND, "No workspace directory").into_response(),
    };
    let git_info = tokio::task::spawn_blocking(move || get_git_info(&cwd))
        .await
        .unwrap_or_else(|_| crate::git::GitInfo {
            branch: String::new(),
            changes: Vec::new(),
            cwd: String::new(),
        });
    Json(git_info).into_response()
}

#[derive(Deserialize)]
pub struct GitDiffsQuery {
    pub base: Option<String>,
    pub meta_only: Option<bool>,
}

/// Get git diffs for a session's workspace
///
/// When `meta_only=true`, returns file list with empty patches (lightweight).
/// Full patches are loaded on demand via `/sessions/{id}/git-diff?file=…`.
#[utoipa::path(
    get,
    path = "/sessions/{id}/git-diffs",
    tag = "sessions",
    params(
        ("id" = String, Path, description = "Session ID"),
    ),
    responses(
        (status = 200, description = "Git diffs"),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn get_session_git_diffs(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<GitDiffsQuery>,
) -> impl IntoResponse {
    let session = match state.session_manager.read().await.get(&id).await {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, "Session not found").into_response(),
    };
    let cwd = match session.cwd {
        Some(ref c) => std::path::PathBuf::from(c),
        None => return (StatusCode::NOT_FOUND, "No workspace directory").into_response(),
    };

    if query.meta_only.unwrap_or(false) {
        // Lightweight: return file list with empty patches
        let meta = tokio::task::spawn_blocking(move || {
            let info = get_git_info(&cwd);
            let branch = info.branch.clone();
            let files: Vec<GitDiffEntry> = info
                .changes
                .iter()
                .map(|c| GitDiffEntry {
                    path: c.path.clone(),
                    status: c.status.clone(),
                    additions: 0,
                    deletions: 0,
                    patch: String::new(),
                    binary: false,
                })
                .collect();
            GitDiffsResponse { branch, files }
        })
        .await
        .unwrap_or_else(|_| GitDiffsResponse {
            branch: String::new(),
            files: Vec::new(),
        });
        return Json(meta).into_response();
    }

    let base = query.base.clone();
    let diffs = tokio::task::spawn_blocking(move || get_git_diffs(&cwd, base.as_deref()))
        .await
        .unwrap_or_else(|_| GitDiffsResponse {
            branch: String::new(),
            files: Vec::new(),
        });
    Json(diffs).into_response()
}

#[derive(Deserialize)]
pub struct GitSingleDiffQuery {
    pub file: String,
}

/// Get a single file's full git diff (for progressive loading).
#[utoipa::path(
    get,
    path = "/sessions/{id}/git-diff",
    tag = "sessions",
    params(
        ("id" = String, Path, description = "Session ID"),
    ),
    responses(
        (status = 200, description = "Single file diff"),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn get_session_single_diff(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<GitSingleDiffQuery>,
) -> impl IntoResponse {
    let session = match state.session_manager.read().await.get(&id).await {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, "Session not found").into_response(),
    };
    let cwd = match session.cwd {
        Some(ref c) => std::path::PathBuf::from(c),
        None => return (StatusCode::NOT_FOUND, "No workspace directory").into_response(),
    };
    let file = query.file.clone();
    let entry = tokio::task::spawn_blocking(move || get_git_single_diff(&cwd, &file))
        .await
        .unwrap_or_default();
    Json(entry).into_response()
}

/// List git branches for a session's workspace
#[utoipa::path(
    get,
    path = "/sessions/{id}/branches",
    tag = "sessions",
    responses(
        (status = 200, description = "List of git branches"),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn get_session_branches(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let session = match state.session_manager.read().await.get(&id).await {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, "Session not found").into_response(),
    };
    let cwd = match session.cwd {
        Some(ref c) => std::path::PathBuf::from(c),
        None => return (StatusCode::NOT_FOUND, "No workspace directory").into_response(),
    };
    let branches = tokio::task::spawn_blocking(move || list_git_branches(&cwd))
        .await
        .unwrap_or_default();
    Json(branches).into_response()
}

/// Get git info for the server's workspace (no session required).
pub async fn get_workspace_git_info(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cwd = resolve_workspace_cwd(&state);
    let git_info = tokio::task::spawn_blocking(move || get_git_info(&cwd))
        .await
        .unwrap_or_else(|_| crate::git::GitInfo {
            branch: String::new(),
            changes: Vec::new(),
            cwd: String::new(),
        });
    Json(git_info).into_response()
}

/// Get git diffs for the server's workspace (no session required).
pub async fn get_workspace_git_diffs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GitDiffsQuery>,
) -> impl IntoResponse {
    let cwd = resolve_workspace_cwd(&state);

    if query.meta_only.unwrap_or(false) {
        let meta = tokio::task::spawn_blocking(move || {
            let info = get_git_info(&cwd);
            let branch = info.branch.clone();
            let files: Vec<GitDiffEntry> = info
                .changes
                .iter()
                .map(|c| GitDiffEntry {
                    path: c.path.clone(),
                    status: c.status.clone(),
                    additions: 0,
                    deletions: 0,
                    patch: String::new(),
                    binary: false,
                })
                .collect();
            GitDiffsResponse { branch, files }
        })
        .await
        .unwrap_or_else(|_| GitDiffsResponse {
            branch: String::new(),
            files: Vec::new(),
        });
        return Json(meta).into_response();
    }

    let base = query.base.clone();
    let diffs = tokio::task::spawn_blocking(move || get_git_diffs(&cwd, base.as_deref()))
        .await
        .unwrap_or_else(|_| GitDiffsResponse {
            branch: String::new(),
            files: Vec::new(),
        });
    Json(diffs).into_response()
}

/// Get a single file's diff for the server's workspace (no session required).
pub async fn get_workspace_git_single_diff(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GitSingleDiffQuery>,
) -> impl IntoResponse {
    let cwd = resolve_workspace_cwd(&state);
    let file = query.file.clone();
    let entry = tokio::task::spawn_blocking(move || get_git_single_diff(&cwd, &file))
        .await
        .unwrap_or_default();
    Json(entry).into_response()
}

/// Get git branches for the server's workspace (no session required).
pub async fn get_workspace_branches(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cwd = resolve_workspace_cwd(&state);
    let branches = tokio::task::spawn_blocking(move || list_git_branches(&cwd))
        .await
        .unwrap_or_default();
    Json(branches).into_response()
}

/// Resolve the server's workspace directory from AppState.
fn resolve_workspace_cwd(state: &AppState) -> std::path::PathBuf {
    state.project_manager.get_cwd()
}

#[derive(Serialize, ToSchema)]
pub struct ServerConfigResponse {
    pub host: String,
    pub port: u16,
}

pub async fn server_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(ServerConfigResponse {
        host: state.config.host.clone(),
        port: state.config.port,
    })
}

#[derive(Deserialize)]
pub struct SetApiKeyRequest {
    pub key: String,
}

/// Set an API key for a provider (persisted to auth.json).
pub async fn set_provider_key(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    Json(req): Json<SetApiKeyRequest>,
) -> impl IntoResponse {
    // Update in-memory map
    state
        .api_keys
        .write()
        .unwrap()
        .insert(provider.clone(), req.key.clone());

    // Persist to auth.json (using AuthFile to preserve last_* fields)
    if let Some(auth_path) = &state.auth_storage_path {
        let mut file = auth_storage::read_auth_file(auth_path).unwrap_or(auth_storage::AuthFile {
            credentials: std::collections::HashMap::new(),
            last_provider: None,
            last_model: None,
            thinking_level: None,
        });
        file.credentials.insert(
            provider.clone(),
            pick_agent::auth::AuthCredential::ApiKey { key: req.key },
        );
        if let Err(e) = auth_storage::write_auth_file(auth_path, &file) {
            tracing::error!("Failed to persist API key to auth.json: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to persist API key",
            )
                .into_response();
        }
    }

    StatusCode::OK.into_response()
}

#[derive(Deserialize)]
pub struct SetLastModelRequest {
    pub provider: String,
    pub model: String,
}

/// Save the last used model (persisted to auth.json).
pub async fn set_last_model(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetLastModelRequest>,
) -> impl IntoResponse {
    // Update in-memory
    *state.last_provider.write().unwrap() = Some(req.provider.clone());
    *state.last_model.write().unwrap() = Some(req.model.clone());

    // Persist to auth.json
    if let Some(auth_path) = &state.auth_storage_path {
        let mut file = auth_storage::read_auth_file(auth_path).unwrap_or(auth_storage::AuthFile {
            credentials: std::collections::HashMap::new(),
            last_provider: None,
            last_model: None,
            thinking_level: None,
        });
        file.last_provider = Some(req.provider);
        file.last_model = Some(req.model);
        if let Err(e) = auth_storage::write_auth_file(auth_path, &file) {
            tracing::error!("Failed to persist last model to auth.json: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to persist last model",
            )
                .into_response();
        }
    }

    StatusCode::OK.into_response()
}

#[derive(Deserialize)]
pub struct SetThinkingLevelRequest {
    pub thinking_level: String,
}

/// Save the thinking level (persisted to auth.json).
pub async fn set_thinking_level(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetThinkingLevelRequest>,
) -> impl IntoResponse {
    // Update in-memory
    *state.thinking_level.write().unwrap() = Some(req.thinking_level.clone());

    // Persist to auth.json
    if let Some(auth_path) = &state.auth_storage_path {
        let mut file = auth_storage::read_auth_file(auth_path).unwrap_or(auth_storage::AuthFile {
            credentials: std::collections::HashMap::new(),
            last_provider: None,
            last_model: None,
            thinking_level: None,
        });
        file.thinking_level = Some(req.thinking_level);
        if let Err(e) = auth_storage::write_auth_file(auth_path, &file) {
            tracing::error!("Failed to persist thinking level to auth.json: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to persist thinking level",
            )
                .into_response();
        }
    }

    StatusCode::OK.into_response()
}
