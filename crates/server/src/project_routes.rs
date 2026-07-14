//! Project management API routes

use std::path::Path;
use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::{AppState, project_manager::ProjectEntry};

#[derive(Deserialize)]
pub struct SetCwdRequest {
    pub cwd: String,
    /// When true, load the project's persisted sessions into memory.
    /// Used when opening a project from the modal (user needs to see
    /// existing sessions).  When false, only update the cwd marker
    /// (used for sidebar project selection).
    #[serde(default)]
    pub load_sessions: bool,
}

#[derive(Serialize)]
pub struct CwdResponse {
    pub cwd: String,
    pub sessions_reloaded: usize,
}

#[derive(Serialize)]
pub struct ProjectListResponse {
    pub projects: Vec<ProjectEntry>,
    pub current_cwd: Option<String>,
}

/// GET /cwd — get the current working directory
pub async fn get_cwd(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cwd = state.project_manager.get_cwd();
    Json(serde_json::json!({ "cwd": cwd.to_string_lossy() }))
}

/// POST /cwd — switch to a different working directory
///
/// Validates the path exists, updates the server's working directory,
/// clears and reloads sessions from the new directory.
pub async fn set_cwd(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetCwdRequest>,
) -> impl IntoResponse {
    let path = Path::new(&req.cwd);

    // Validate path exists
    if !path.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Path does not exist: {}", req.cwd)
            })),
        )
            .into_response();
    }

    // Canonicalize path
    let canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Failed to resolve path: {}", e)
                })),
            )
                .into_response();
        }
    };
    let cwd_str = canonical.to_string_lossy().to_string();

    // 2. Update OS-level current directory so all spawned processes
    //    inherit the correct working directory.
    if let Err(e) = std::env::set_current_dir(&canonical) {
        debug!("Failed to set OS current directory: {}", e);
    }

    // 3. Update SessionManager cwd + session_dir
    state
        .session_manager
        .write()
        .await
        .set_cwd(canonical.clone());

    // 4. Update ProjectManager (persists to projects.json and tracks current cwd)
    if let Err(e) = state.project_manager.set_cwd(&canonical) {
        debug!("Failed to persist project: {}", e);
    }

    // 5. Optionally load persisted sessions from the new project directory.
    //    This is needed when the project is opened from the modal so that
    //    existing sessions appear in the sidebar.
    if req.load_sessions {
        let project_session_dir = canonical.join(".pick").join("sessions");
        if project_session_dir.exists() {
            debug!("Loading sessions from: {}", project_session_dir.display());
            state
                .load_single_session_dir(&project_session_dir, &canonical)
                .await;
        }
        // Also pick up global sessions that belong to this project
        if let Some(home) = AppState::home_dir() {
            let global_dir = home.join(".pick").join("agent").join("sessions");
            if global_dir.exists() {
                state.load_global_session_dir(&global_dir).await;
            }
        }
    }

    debug!("Switched working directory to {}", cwd_str);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "cwd": cwd_str,
        })),
    )
        .into_response()
}

/// GET /projects — list all historically used projects
pub async fn list_projects(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let projects = state.project_manager.list_projects().unwrap_or_default();
    let current_cwd = state
        .project_manager
        .get_cwd()
        .to_string_lossy()
        .to_string();
    Json(ProjectListResponse {
        projects,
        current_cwd: Some(current_cwd),
    })
}

#[derive(Deserialize)]
pub struct AddProjectRequest {
    pub cwd: String,
}

/// POST /projects — add a project to history (without switching)
pub async fn add_project(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddProjectRequest>,
) -> impl IntoResponse {
    let path = Path::new(&req.cwd);
    if !path.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Path does not exist" })),
        )
            .into_response();
    }

    if let Err(e) = state.project_manager.add_project(path) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to save: {}", e) })),
        )
            .into_response();
    }

    Json(serde_json::json!({ "status": "ok" })).into_response()
}

#[derive(Deserialize)]
pub struct RemoveProjectRequest {
    pub cwd: String,
}

/// POST /projects/remove — remove a project from history (keeps session files on disk)
pub async fn remove_project(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RemoveProjectRequest>,
) -> impl IntoResponse {
    if let Err(e) = state.project_manager.remove_project(&req.cwd) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to remove: {}", e) })),
        )
            .into_response();
    }

    Json(serde_json::json!({ "status": "ok" })).into_response()
}
