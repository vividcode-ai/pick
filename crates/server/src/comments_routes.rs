use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentEntry {
    pub id: String,
    pub file: String,
    pub line: i64,
    pub comment: String,
    pub time: i64,
    pub resolved: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommentsBody {
    pub comments: Vec<CommentEntry>,
}

#[derive(Debug, Deserialize)]
pub struct CommentsQuery {
    pub session_id: Option<String>,
}

/// GET /comments/{session_id} — Load comments for a session
pub async fn get_comments(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let store = state.comments.read().await;
    let comments = store.get(&session_id).cloned().unwrap_or_default();
    (StatusCode::OK, Json(CommentsBody { comments })).into_response()
}

/// PUT /comments/{session_id} — Save comments for a session
pub async fn put_comments(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(body): Json<CommentsBody>,
) -> impl IntoResponse {
    let mut store = state.comments.write().await;
    store.insert(session_id, body.comments);
    (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
}
