use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::AppState;

#[derive(Debug, Serialize)]
pub struct LoopJobResponse {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub status: String,
    pub action: String,
    pub interval_ms: u64,
    pub run_count: u32,
    pub max_runs: Option<u32>,
    pub failure_count: u32,
    pub max_failures: Option<u32>,
    pub next_due_ms: i64,
    pub last_run_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateLoopRequest {
    pub name: Option<String>,
    pub action: String,
    pub kind: Option<String>,
    pub interval_ms: u64,
    pub immediate: Option<bool>,
    pub max_runs: Option<u32>,
    pub max_failures: Option<u32>,
    pub verify_command: Option<String>,
    pub preflight_command: Option<String>,
    pub postrun_command: Option<String>,
    pub safe: Option<bool>,
    pub quiet: Option<bool>,
    pub ask_never: Option<bool>,
    pub git_checkpoint: Option<bool>,
    pub branch: Option<String>,
}

/// GET /sessions/{id}/loops — list all loop jobs
pub async fn list_loops(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let sse_state = match state.sse_sessions.read().await.get(&session_id).cloned() {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, "Session not found").into_response(),
    };
    let mgr = sse_state.loop_manager.read().await;
    let jobs: Vec<LoopJobResponse> = mgr
        .list()
        .iter()
        .map(|j| LoopJobResponse {
            id: j.id.clone(),
            name: j.name.clone(),
            kind: j.kind.clone(),
            status: j.status.label().to_string(),
            action: j.action.clone(),
            interval_ms: j.interval_ms,
            run_count: j.run_count,
            max_runs: j.max_runs,
            failure_count: j.failure_count,
            max_failures: j.max_failures,
            next_due_ms: j.due_in_ms(chrono::Utc::now().timestamp_millis()),
            last_run_at: j.last_run_at,
            created_at: j.created_at,
        })
        .collect();
    Json(jobs).into_response()
}

/// POST /sessions/{id}/loops — create a loop job
pub async fn create_loop(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(req): Json<CreateLoopRequest>,
) -> impl IntoResponse {
    let sse_state = match state.sse_sessions.read().await.get(&session_id).cloned() {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, "Session not found").into_response(),
    };

    let kind = req.kind.as_deref().unwrap_or("prompt");
    let name = req.name.unwrap_or_else(|| {
        if req.action.len() > 27 {
            format!("{}...", req.action.chars().take(27).collect::<String>())
        } else {
            req.action.clone()
        }
    });

    let mut job = pick_loop::LoopJob::new_prompt(
        uuid::Uuid::now_v7().to_string(),
        name,
        req.action,
        req.interval_ms,
        req.immediate.unwrap_or(true),
    );
    job.kind = kind.to_string();
    job.max_runs = req.max_runs;
    job.max_failures = req.max_failures;
    job.verify_command = req.verify_command;
    job.preflight_command = req.preflight_command;
    job.postrun_command = req.postrun_command;
    job.safe = req.safe.unwrap_or(false);
    job.quiet = req.quiet.unwrap_or(false);
    job.ask_never = req.ask_never.unwrap_or(false);
    job.git_checkpoint = req.git_checkpoint.unwrap_or(false);
    job.branch = req.branch;

    let job_id = {
        let mut mgr = sse_state.loop_manager.write().await;
        let id = mgr.create(job);
        let _ = mgr.save();
        id
    };

    // Schedule and optionally trigger immediately
    let schedule_immediate = req.immediate.unwrap_or(true);
    {
        let mgr = sse_state.loop_manager.read().await;
        if let Some(job) = mgr.get(&job_id).cloned() {
            sse_state.loop_scheduler.schedule(&job).await;
            if schedule_immediate || req.interval_ms == 0 {
                sse_state.loop_scheduler.trigger_job(&job_id).await;
            }
        }
    }

    // Send SSE event
    let event = serde_json::json!({"job_id": job_id});
    let _ = sse_state
        .event_tx
        .send(Ok(axum::response::sse::Event::default()
            .event("loop_created")
            .data(serde_json::to_string(&event).unwrap_or_default())));

    debug!("Loop job created: {}", job_id);
    (StatusCode::CREATED, Json(serde_json::json!({"id": job_id}))).into_response()
}

/// DELETE /sessions/{id}/loops/{job_id} — remove a loop job
pub async fn delete_loop(
    State(state): State<Arc<AppState>>,
    Path((session_id, job_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let sse_state = match state.sse_sessions.read().await.get(&session_id).cloned() {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, "Session not found").into_response(),
    };

    let removed = {
        let mut mgr = sse_state.loop_manager.write().await;
        let r = mgr.remove(&job_id);
        if r {
            let _ = mgr.save();
        }
        r
    };

    if removed {
        sse_state.loop_scheduler.deschedule(&job_id).await;
        let event = serde_json::json!({"job_id": job_id});
        let _ = sse_state
            .event_tx
            .send(Ok(axum::response::sse::Event::default()
                .event("loop_deleted")
                .data(serde_json::to_string(&event).unwrap_or_default())));
        (StatusCode::OK, "Removed").into_response()
    } else {
        (StatusCode::NOT_FOUND, "Job not found").into_response()
    }
}

/// POST /sessions/{id}/loops/{job_id}/pause
pub async fn pause_loop(
    State(state): State<Arc<AppState>>,
    Path((session_id, job_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let sse_state = match state.sse_sessions.read().await.get(&session_id).cloned() {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, "Session not found").into_response(),
    };

    let result = {
        let mut mgr = sse_state.loop_manager.write().await;
        let r = mgr.pause(&job_id);
        if r.is_ok() {
            let _ = mgr.save();
        }
        r
    };

    match result {
        Ok(()) => {
            sse_state.loop_scheduler.deschedule(&job_id).await;
            send_loop_update(&sse_state).await;
            (StatusCode::OK, "Paused").into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}

/// POST /sessions/{id}/loops/{job_id}/resume
pub async fn resume_loop(
    State(state): State<Arc<AppState>>,
    Path((session_id, job_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let sse_state = match state.sse_sessions.read().await.get(&session_id).cloned() {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, "Session not found").into_response(),
    };

    let result = {
        let mut mgr = sse_state.loop_manager.write().await;
        let r = mgr.resume(&job_id);
        if r.is_ok() {
            let _ = mgr.save();
        }
        r
    };

    match result {
        Ok(()) => {
            let mgr = sse_state.loop_manager.read().await;
            if let Some(job) = mgr.get(&job_id).cloned() {
                sse_state.loop_scheduler.schedule(&job).await;
            }
            send_loop_update(&sse_state).await;
            (StatusCode::OK, "Resumed").into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}

/// POST /sessions/{id}/loops/{job_id}/trigger
pub async fn trigger_loop(
    State(state): State<Arc<AppState>>,
    Path((session_id, job_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let sse_state = match state.sse_sessions.read().await.get(&session_id).cloned() {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, "Session not found").into_response(),
    };

    sse_state.loop_scheduler.trigger_job(&job_id).await;
    (StatusCode::OK, "Triggered").into_response()
}

/// POST /sessions/{id}/loops/clear
pub async fn clear_loops(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let sse_state = match state.sse_sessions.read().await.get(&session_id).cloned() {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, "Session not found").into_response(),
    };

    {
        let mut mgr = sse_state.loop_manager.write().await;
        mgr.clear();
        let _ = mgr.save();
    }
    sse_state.loop_scheduler.deschedule_all().await;
    send_loop_update(&sse_state).await;
    (StatusCode::OK, "Cleared").into_response()
}

/// Send a loop_updated SSE event with current state.
async fn send_loop_update(sse_state: &crate::session::SseSessionState) {
    let mgr = sse_state.loop_manager.read().await;
    let jobs: Vec<LoopJobResponse> = mgr
        .list()
        .iter()
        .map(|j| LoopJobResponse {
            id: j.id.clone(),
            name: j.name.clone(),
            kind: j.kind.clone(),
            status: j.status.label().to_string(),
            action: j.action.clone(),
            interval_ms: j.interval_ms,
            run_count: j.run_count,
            max_runs: j.max_runs,
            failure_count: j.failure_count,
            max_failures: j.max_failures,
            next_due_ms: j.due_in_ms(chrono::Utc::now().timestamp_millis()),
            last_run_at: j.last_run_at,
            created_at: j.created_at,
        })
        .collect();
    let payload = serde_json::json!({"jobs": jobs});
    let _ = sse_state
        .event_tx
        .send(Ok(axum::response::sse::Event::default()
            .event("loop_updated")
            .data(serde_json::to_string(&payload).unwrap_or_default())));
}
