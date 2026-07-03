use std::collections::HashMap;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use pick_agent::core::message_queue::{PendingMessageQueue, QueueMode};
use serde::Deserialize;
use tokio::sync::mpsc;
use tracing::debug;

use crate::AppState;
use crate::git::get_git_info;
use crate::session::SseSessionState;

#[derive(Deserialize)]
pub struct SseQuery {
    pub mode: Option<String>,
}

struct SseStream {
    rx: mpsc::UnboundedReceiver<Result<Event, Infallible>>,
    session_id: String,
    state: Arc<AppState>,
}

impl futures::stream::Stream for SseStream {
    type Item = Result<Event, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}

impl Drop for SseStream {
    fn drop(&mut self) {
        let state = self.state.clone();
        let session_id = self.session_id.clone();
        tokio::spawn(async move {
            let mut sessions = state.sse_sessions.write().await;
            sessions.remove(&session_id);
        });
    }
}

pub async fn handle_sse(
    Path(session_id): Path<String>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<SseQuery>,
) -> impl IntoResponse {
    let session = state.session_manager.get(&session_id).await;
    if session.is_none() {
        return (StatusCode::NOT_FOUND, "Session not found").into_response();
    }

    let mode = query.mode.unwrap_or_else(|| "build".to_string());

    let (tx, rx) = mpsc::unbounded_channel::<Result<Event, Infallible>>();

    {
        let mut sessions = state.sse_sessions.write().await;
        sessions.insert(
            session_id.clone(),
            SseSessionState {
                event_tx: tx.clone(),
                cancel_tx: None,
                pending_approvals: Arc::new(Mutex::new(HashMap::new())),
                pending_questions: Arc::new(Mutex::new(HashMap::new())),
                message_queue: Arc::new(Mutex::new(PendingMessageQueue::new(
                    QueueMode::OneAtATime,
                ))),
                in_flight: Arc::new(AtomicBool::new(false)),
                agent_mode: Arc::new(std::sync::RwLock::new(mode)),
            },
        );
    }

    // Send initial git info
    if let Some(s) = state.session_manager.get(&session_id).await {
        let cwd = s.cwd.and_then(|c| {
            let p = std::path::PathBuf::from(&c);
            if p.exists() { Some(p) } else { None }
        });
        if let Some(cwd) = cwd {
            let git_info = get_git_info(&cwd);
            if let Ok(payload) = serde_json::to_string(&git_info) {
                let _ = tx.send(Ok(Event::default().event("git_info_updated").data(payload)));
            }
        }
    }

    let stream = SseStream {
        rx,
        session_id: session_id.clone(),
        state: state.clone(),
    };

    let sse = Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("heartbeat"),
    );

    debug!("SSE connected for session {}", session_id);
    sse.into_response()
}
