use std::collections::HashMap;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use tokio::sync::mpsc;
use tracing::info;

use crate::AppState;
use crate::session::SseSessionState;

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
            if let Some(s) = sessions.remove(&session_id)
                && let Some(cancel_tx) = s.cancel_tx
            {
                let _ = cancel_tx.send(true);
            }
        });
    }
}

pub async fn handle_sse(
    Path(session_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let session = state.session_manager.get(&session_id).await;
    if session.is_none() {
        return (StatusCode::NOT_FOUND, "Session not found").into_response();
    }

    let (tx, rx) = mpsc::unbounded_channel::<Result<Event, Infallible>>();

    {
        let mut sessions = state.sse_sessions.write().await;
        sessions.insert(
            session_id.clone(),
            SseSessionState {
                event_tx: tx,
                cancel_tx: None,
                pending_approvals: Arc::new(std::sync::Mutex::new(HashMap::new())),
                pending_questions: Arc::new(std::sync::Mutex::new(HashMap::new())),
            },
        );
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

    info!("SSE connected for session {}", session_id);
    sse.into_response()
}
