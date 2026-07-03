use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use pick_agent::prompt_history::HistoryProvider;
use serde::{Deserialize, Serialize};

use crate::AppState;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct HistoryWindowResponse {
    pub window: Vec<String>,
}

#[derive(Serialize)]
pub struct NavigateResponse {
    /// The history entry text, or `None` if navigation failed.
    pub text: Option<String>,
    /// Whether the user is still in history browsing mode.
    pub browsing: bool,
    /// Updated window after navigation.
    pub window: Vec<String>,
}

#[derive(Deserialize)]
pub struct NavigateRequest {
    pub direction: String, // "up" or "down"
    pub current_input: String,
}

#[derive(Deserialize)]
pub struct PushRequest {
    pub text: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /prompt-history
///
/// Returns the current in-memory history window.
pub async fn get_history_window(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let history = state.prompt_history.lock().await;
    Json(HistoryWindowResponse {
        window: history.window().to_vec(),
    })
}

/// POST /prompt-history/navigate
///
/// Navigate history in the given direction.
/// - `direction: "up"` → go older
/// - `direction: "down"` → go newer
///   Returns the entry text (or `None`) and the updated window.
pub async fn navigate_history(
    State(state): State<Arc<AppState>>,
    Json(req): Json<NavigateRequest>,
) -> impl IntoResponse {
    let mut history = state.prompt_history.lock().await;
    let text = match req.direction.as_str() {
        "up" => history.previous(&req.current_input),
        "down" => history.next(&req.current_input),
        _ => return (StatusCode::BAD_REQUEST, "Invalid direction").into_response(),
    };
    let browsing = history.is_browsing();
    let window = history.window().to_vec();
    Json(NavigateResponse {
        text,
        browsing,
        window,
    })
    .into_response()
}

/// POST /prompt-history/push
///
/// Append a submitted message to history.
pub async fn push_history(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PushRequest>,
) -> impl IntoResponse {
    let mut history = state.prompt_history.lock().await;
    history.push(&req.text);
    let window = history.window().to_vec();
    Json(HistoryWindowResponse { window })
}
