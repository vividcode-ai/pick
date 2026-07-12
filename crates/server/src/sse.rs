use std::collections::HashMap;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
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

    let message_queue: Arc<Mutex<PendingMessageQueue>> =
        Arc::new(Mutex::new(PendingMessageQueue::new(QueueMode::OneAtATime)));

    // Initialize loop components
    let cwd_path = {
        let s = state.session_manager.get(&session_id).await;
        s.and_then(|s| s.cwd.map(std::path::PathBuf::from))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
    };
    let loops_path = pick_loop::manager::loops_path_for_session(&cwd_path, &session_id);
    let loop_manager = Arc::new(tokio::sync::RwLock::new(pick_loop::LoopManager::load(
        &loops_path,
    )));

    // Create loop wakeup signal
    let loop_wakeup = Arc::new(tokio::sync::Notify::new());

    // Build trigger callback
    let loop_mq = message_queue.clone();
    let loop_event_tx = tx.clone();
    let loop_mgr_cb = loop_manager.clone();
    let loop_wakeup_cb = loop_wakeup.clone();
    let trigger_cb: pick_loop::scheduler::TriggerCallback = Arc::new(move |job| {
        let mq = loop_mq.clone();
        let et = loop_event_tx.clone();
        let mgr = loop_mgr_cb.clone();
        let wakeup = loop_wakeup_cb.clone();
        Box::pin(async move {
            let is_shell_or_command = job.kind == "shell" || job.kind == "command";

            if is_shell_or_command {
                // Execute shell/command directly (e.g. npm test, /compact)
                let _ = tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(&job.action)
                    .output()
                    .await;
            } else {
                // Enqueue message for agent (prompt, goal, ask)
                let msg = pick_loop::integration::build_loop_message(&job);
                if let Ok(mut q) = mq.lock() {
                    q.enqueue(msg);
                }
                // Wake up the agent loop so it processes the new message immediately
                wakeup.notify_one();
            }
            // Send full loop state
            let all_info: Vec<pick_loop::types::LoopJobStatusInfo> = {
                let m = mgr.read().await;
                m.list()
                    .iter()
                    .map(pick_loop::types::LoopJobStatusInfo::from)
                    .collect()
            };
            let payload = serde_json::json!({"jobs": all_info});
            let _ = et.send(Ok(Event::default()
                .event("loop_updated")
                .data(serde_json::to_string(&payload).unwrap_or_default())));
            // Send execution start event
            let exec = serde_json::json!({
                "job_id": job.id, "job_name": job.name,
                "run_count": job.run_count + 1, "max_runs": job.max_runs,
            });
            let _ = et.send(Ok(Event::default()
                .event("loop_execution_start")
                .data(serde_json::to_string(&exec).unwrap_or_default())));
        })
    });

    let mut loop_scheduler = pick_loop::LoopScheduler::new(loop_manager.clone());
    loop_scheduler.set_trigger_cb(trigger_cb);
    loop_scheduler.start_watchdog();
    let loop_scheduler = Arc::new(loop_scheduler);

    {
        let mut sessions = state.sse_sessions.write().await;
        sessions.insert(
            session_id.clone(),
            SseSessionState {
                event_tx: tx.clone(),
                cancel_tx: None,
                pending_approvals: Arc::new(Mutex::new(HashMap::new())),
                pending_questions: Arc::new(Mutex::new(HashMap::new())),
                message_queue,
                in_flight: Arc::new(AtomicBool::new(false)),
                agent_mode: Arc::new(std::sync::RwLock::new(mode)),
                goal_manager: Arc::new(RwLock::new(None)),
                loop_manager,
                loop_scheduler,
                loop_wakeup,
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
