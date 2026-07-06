use std::path::Path;
use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};

use crate::AppState;

fn read_file(path: &Path) -> String {
    if path.exists() {
        std::fs::read_to_string(path).unwrap_or_default()
    } else {
        String::new()
    }
}

fn scope_path(cwd: &Path, agent_dir: &Path, scope: &str, filename: &str) -> std::path::PathBuf {
    if scope == "global" {
        agent_dir.join(filename)
    } else {
        cwd.join(pick_agent::system_prompt::CONFIG_DIR_NAME)
            .join(filename)
    }
}

#[derive(Serialize)]
struct PromptScope {
    project: String,
    global: String,
}

#[derive(Serialize)]
struct PromptsResponse {
    system_prompt: PromptScope,
    append_prompt: PromptScope,
}

#[derive(Deserialize)]
pub(super) struct PromptsUpdate {
    system_prompt: Option<String>,
    append_prompt: Option<String>,
    #[serde(default)]
    scope: String,
}

/// GET /agent/prompts — return SYSTEM.md and APPEND_SYSTEM.md content for both scopes
pub(super) async fn get_prompts(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cwd = state
        .config
        .cwd
        .as_deref()
        .map(Path::new)
        .unwrap_or_else(|| Path::new("."));
    let agent_dir = pick_agent::system_prompt::get_agent_dir();

    Json(PromptsResponse {
        system_prompt: PromptScope {
            project: read_file(
                &cwd.join(pick_agent::system_prompt::CONFIG_DIR_NAME)
                    .join("SYSTEM.md"),
            ),
            global: read_file(&agent_dir.join("SYSTEM.md")),
        },
        append_prompt: PromptScope {
            project: read_file(
                &cwd.join(pick_agent::system_prompt::CONFIG_DIR_NAME)
                    .join("APPEND_SYSTEM.md"),
            ),
            global: read_file(&agent_dir.join("APPEND_SYSTEM.md")),
        },
    })
}

/// PUT /agent/prompts — write SYSTEM.md and/or APPEND_SYSTEM.md
/// scope: "project" (default) or "global"
pub(super) async fn update_prompts(
    State(state): State<Arc<AppState>>,
    Json(update): Json<PromptsUpdate>,
) -> impl IntoResponse {
    let cwd = state
        .config
        .cwd
        .as_deref()
        .map(Path::new)
        .unwrap_or_else(|| Path::new("."));
    let agent_dir = pick_agent::system_prompt::get_agent_dir();
    let scope = if update.scope == "global" {
        "global"
    } else {
        "project"
    };

    if let Some(content) = update.system_prompt {
        let path = scope_path(cwd, &agent_dir, scope, "SYSTEM.md");
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
        let path = scope_path(cwd, &agent_dir, scope, "APPEND_SYSTEM.md");
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
