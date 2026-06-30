use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Serialize)]
pub struct SessionListResponse {
    pub sessions: Vec<String>,
}

#[derive(Deserialize)]
pub struct CreateSessionRequest {
    pub model_id: Option<String>,
    pub provider: Option<String>,
}

#[derive(Serialize)]
pub struct CreateSessionResponse {
    pub session_id: String,
}

pub async fn health() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

pub async fn list_sessions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let sessions = state.session_manager.list().await;
    Json(SessionListResponse { sessions })
}

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

    let session_id = state
        .session_manager
        .create(model_id, provider, system_prompt, tools)
        .await;

    (
        StatusCode::CREATED,
        Json(CreateSessionResponse { session_id }),
    )
}

pub async fn get_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let session = state.session_manager.get(&id).await;
    match session {
        Some(s) => Ok(Json(serde_json::json!({
            "id": s.id,
            "model_id": s.model_id,
            "provider": s.provider,
            "message_count": s.messages.len(),
        }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

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

#[derive(Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub reasoning: bool,
}

#[derive(Serialize)]
pub struct ProviderInfo {
    pub provider: String,
    pub models: Vec<ModelInfo>,
}

pub async fn list_providers() -> impl IntoResponse {
    let registry = pick_ai::registry::global_registry();
    let apis = registry.list_apis();
    let providers: Vec<ProviderInfo> = apis
        .iter()
        .map(|p| {
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
                models,
            }
        })
        .collect();
    Json(providers)
}
