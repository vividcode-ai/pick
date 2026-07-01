use std::collections::HashMap;
use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{info, warn};

/// Manages PTY (terminal) sessions
pub struct PtyManager {
    sessions: Arc<RwLock<HashMap<String, PtySession>>>,
}

impl Clone for PtyManager {
    fn clone(&self) -> Self {
        Self {
            sessions: self.sessions.clone(),
        }
    }
}

#[allow(dead_code)]
struct PtySession {
    child: Arc<Mutex<Child>>,
    created_at: i64,
    shell: String,
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create(&self) -> String {
        let id = uuid::Uuid::now_v7().to_string();
        let now = chrono::Utc::now().timestamp_millis();
        let shell = if cfg!(target_os = "windows") {
            "cmd.exe"
        } else {
            "bash"
        };

        let child = match Command::new(shell)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to spawn shell {}: {}", shell, e);
                return String::new();
            }
        };

        let session = PtySession {
            child: Arc::new(Mutex::new(child)),
            created_at: now,
            shell: shell.to_string(),
        };
        self.sessions.write().await.insert(id.clone(), session);
        info!("PTY session {} created with shell {}", id, shell);
        id
    }

    pub async fn remove(&self, id: &str) -> bool {
        if let Some(session) = self.sessions.write().await.remove(id) {
            let mut child = session.child.lock().await;
            let _ = child.kill().await;
            let _ = child.wait().await;
            info!("PTY session {} destroyed", id);
            true
        } else {
            false
        }
    }

    pub async fn list(&self) -> Vec<String> {
        self.sessions.read().await.keys().cloned().collect()
    }
}

/// Start the WebSocket PTY server on a background task
pub async fn start_pty_ws_server(
    _pty_manager: Arc<PtyManager>,
    port: u16,
) -> Result<(), std::io::Error> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    info!("PTY WebSocket server listening on {}", addr);

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                info!("PTY WebSocket connection from {}", peer_addr);
                tokio::spawn(async move {
                    if let Err(e) = handle_pty_connection(stream).await {
                        warn!("PTY WebSocket error: {}", e);
                    }
                });
            }
            Err(e) => {
                warn!("PTY accept error: {}", e);
            }
        }
    }
}

async fn handle_pty_connection(
    stream: tokio::net::TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    let shell = if cfg!(target_os = "windows") {
        "cmd.exe"
    } else {
        "bash"
    };

    let mut child = Command::new(shell)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // Welcome message
    let welcome = format!(
        "\r\n*** Pick Terminal ({}): Type 'exit' to close ***\r\n",
        shell
    );
    let _ = ws_sender.send(Message::Text(welcome)).await;

    // Use a channel to merge stdout/stderr into the WebSocket sender
    let (output_tx, mut output_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

    // Task: read shell stdout → channel
    let tx = output_tx.clone();
    let stdout_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            let n = reader.read_until(b'\n', &mut buf).await.unwrap_or(0);
            if n == 0 {
                break;
            }
            if tx.send(buf.clone()).is_err() {
                break;
            }
        }
    });

    // Task: read shell stderr → channel
    let tx = output_tx.clone();
    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            let n = reader.read_until(b'\n', &mut buf).await.unwrap_or(0);
            if n == 0 {
                break;
            }
            if tx.send(buf.clone()).is_err() {
                break;
            }
        }
    });

    // Task: forward channel → WebSocket sender
    let ws_write_task = tokio::spawn(async move {
        while let Some(data) = output_rx.recv().await {
            if ws_sender.send(Message::Binary(data)).await.is_err() {
                break;
            }
        }
    });

    // Task: read from WebSocket receiver → shell stdin
    let ws_read_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Text(text) => {
                    if text.trim() == "exit" {
                        break;
                    }
                    let _ = stdin.write_all(text.as_bytes()).await;
                    let _ = stdin.write_all(b"\n").await;
                    let _ = stdin.flush().await;
                }
                Message::Binary(data) => {
                    let _ = stdin.write_all(&data).await;
                    let _ = stdin.flush().await;
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = stdout_task => {},
        _ = stderr_task => {},
        _ = ws_write_task => {},
        _ = ws_read_task => {},
    }

    let _ = child.kill().await;
    let _ = child.wait().await;
    info!("PTY connection closed");
    Ok(())
}

#[utoipa::path(
    post,
    path = "/pty",
    tag = "pty",
    responses(
        (status = 201, description = "PTY session created, returns session ID"),
        (status = 500, description = "Failed to create PTY"),
    ),
)]
/// REST handler: create PTY session
pub async fn create_pty_handler(
    axum::extract::State(state): axum::extract::State<Arc<crate::AppState>>,
) -> impl IntoResponse {
    let id = state.pty_manager.create().await;
    if id.is_empty() {
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create PTY").into_response()
    } else {
        (StatusCode::CREATED, id).into_response()
    }
}

#[utoipa::path(
    get,
    path = "/pty",
    tag = "pty",
    responses(
        (status = 200, description = "List of PTY session IDs"),
    ),
)]
/// REST handler: list PTY sessions
pub async fn list_pty_handler(
    axum::extract::State(state): axum::extract::State<Arc<crate::AppState>>,
) -> impl IntoResponse {
    let sessions = state.pty_manager.list().await;
    serde_json::json!({ "sessions": sessions }).to_string()
}

#[utoipa::path(
    delete,
    path = "/pty/{id}",
    tag = "pty",
    responses(
        (status = 204, description = "PTY session destroyed"),
        (status = 404, description = "PTY session not found"),
    ),
    params(
        ("id" = String, Path, description = "PTY session ID"),
    ),
)]
/// REST handler: destroy PTY session
pub async fn destroy_pty_handler(
    axum::extract::State(state): axum::extract::State<Arc<crate::AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> StatusCode {
    if state.pty_manager.remove(&id).await {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}
