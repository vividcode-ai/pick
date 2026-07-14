use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};

use crate::AppState;

/// GET /settings — returns the merged settings (global + project)
pub async fn get_settings(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cwd = state.project_manager.get_cwd();

    let sm = pick_agent::settings::SettingsManager::load_from_paths(
        pick_agent::settings::get_global_settings_path(),
        pick_agent::settings::get_project_settings_path(&cwd),
    );

    Json(sm.get().clone())
}

/// PATCH /settings — merge partial settings into global settings
pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(update): Json<pick_agent::settings::Settings>,
) -> impl IntoResponse {
    let cwd = state.project_manager.get_cwd();

    let mut sm = pick_agent::settings::SettingsManager::load_from_paths(
        pick_agent::settings::get_global_settings_path(),
        pick_agent::settings::get_project_settings_path(&cwd),
    );

    match sm.set_global(update) {
        Ok(()) => Json(serde_json::json!({"status": "ok"})).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}
