use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Serialize)]
struct PromptsResponse {
    system_prompt: String,
    append_prompt: String,
}

#[derive(Deserialize)]
pub(super) struct PromptsUpdate {
    system_prompt: Option<String>,
    append_prompt: Option<String>,
}

/// GET /agent/prompts — return current SYSTEM.md and APPEND_SYSTEM.md content
pub(super) async fn get_prompts(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cwd = state
        .config
        .cwd
        .as_deref()
        .map(std::path::Path::new)
        .unwrap_or_else(|| std::path::Path::new("."));
    let agent_dir = pick_agent::system_prompt::get_agent_dir();

    let system_prompt =
        pick_agent::system_prompt::discover_custom_prompt(&agent_dir, cwd).unwrap_or_default();
    let append_prompt =
        pick_agent::system_prompt::discover_append_prompt(&agent_dir, cwd).join("\n");

    Json(PromptsResponse {
        system_prompt,
        append_prompt,
    })
}

/// PUT /agent/prompts — write SYSTEM.md and/or APPEND_SYSTEM.md (project-level only)
pub(super) async fn update_prompts(
    State(state): State<Arc<AppState>>,
    Json(update): Json<PromptsUpdate>,
) -> impl IntoResponse {
    let cwd = state
        .config
        .cwd
        .as_deref()
        .map(std::path::Path::new)
        .unwrap_or_else(|| std::path::Path::new("."));

    if let Some(content) = update.system_prompt {
        let path = cwd
            .join(pick_agent::system_prompt::CONFIG_DIR_NAME)
            .join("SYSTEM.md");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&path, &content) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to write SYSTEM.md: {}", e),
            )
                .into_response();
        }
    }

    if let Some(content) = update.append_prompt {
        let path = cwd
            .join(pick_agent::system_prompt::CONFIG_DIR_NAME)
            .join("APPEND_SYSTEM.md");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&path, &content) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to write APPEND_SYSTEM.md: {}", e),
            )
                .into_response();
        }
    }

    Json(serde_json::json!({"status": "ok"})).into_response()
}
