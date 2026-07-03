use std::collections::HashMap;
use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, warn};

/// Manages PTY (terminal) sessions
pub struct PtyManager {
    sessions: Arc<RwLock<HashMap<String, PtySession>>>,
    cwd: Option<String>,
}

impl Clone for PtyManager {
    fn clone(&self) -> Self {
        Self {
            sessions: self.sessions.clone(),
            cwd: self.cwd.clone(),
        }
    }
}

#[allow(dead_code)]
struct PtySession {
    child: Arc<Mutex<std::process::Child>>,
    created_at: i64,
    shell: String,
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new(None)
    }
}

impl PtyManager {
    pub fn new(cwd: Option<String>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            cwd,
        }
    }

    pub fn cwd(&self) -> Option<&str> {
        self.cwd.as_deref()
    }

    pub async fn create(&self) -> String {
        let id = uuid::Uuid::now_v7().to_string();
        let now = chrono::Utc::now().timestamp_millis();
        let shell = if cfg!(target_os = "windows") {
            "cmd.exe"
        } else {
            "bash"
        };

        let child = match spawn_shell_piped(shell, self.cwd.as_deref()) {
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
        debug!("PTY session {} created with shell {}", id, shell);
        id
    }

    pub async fn remove(&self, id: &str) -> bool {
        if let Some(session) = self.sessions.write().await.remove(id) {
            let mut child = session.child.lock().await;
            let _ = child.kill();
            let _ = child.wait();
            debug!("PTY session {} destroyed", id);
            true
        } else {
            false
        }
    }

    pub async fn list(&self) -> Vec<String> {
        self.sessions.read().await.keys().cloned().collect()
    }
}

fn spawn_shell_piped(
    shell: &str,
    cwd: Option<&str>,
) -> Result<std::process::Child, std::io::Error> {
    let mut cmd = std::process::Command::new(shell);
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    cmd.spawn()
}

/// Start the WebSocket PTY server on a background task
pub async fn start_pty_ws_server(pty_manager: PtyManager, port: u16) -> Result<(), std::io::Error> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    debug!("PTY WebSocket server listening on {}", addr);

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                debug!("PTY WebSocket connection from {}", peer_addr);
                let cwd = pty_manager.cwd().map(|s| s.to_string());
                tokio::spawn(async move {
                    if let Err(e) = handle_pty_connection(stream, cwd).await {
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
    cwd: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    let shell = if cfg!(target_os = "windows") {
        "cmd.exe"
    } else {
        "bash"
    };

    // Spawn shell with piped stdio (uses tokio process for async I/O)
    let mut cmd = tokio::process::Command::new(shell);
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(ref cwd) = cwd {
        cmd.current_dir(cwd);
    }
    let mut child = cmd.spawn()?;

    let mut pty_input = child.stdin.take().ok_or("shell has no stdin")?;
    let pty_output = child.stdout.take().ok_or("shell has no stdout")?;
    let pty_err = child.stderr.take().ok_or("shell has no stderr")?;

    // Welcome message
    let welcome = format!(
        "\r\n*** Pick Terminal ({}): Type 'exit' to close ***\r\n",
        shell
    );
    let _ = ws_sender.send(Message::Text(welcome)).await;

    // ── Set up I/O channels ──────────────────────────────────────────
    let (output_tx, mut output_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let (input_tx, mut input_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

    // ── I/O tasks ────────────────────────────────────────────────────

    // Read from stdout → output channel
    let read_tx = output_tx.clone();
    let rh_out = tokio::spawn(async move {
        let mut buf = vec![0u8; 8192];
        let mut reader = tokio::io::BufReader::new(pty_output);
        loop {
            buf.clear();
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(_) => {
                    if read_tx.send(buf.clone()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Read from stderr → output channel
    let err_tx = output_tx.clone();
    let rh_err = tokio::spawn(async move {
        let mut buf = vec![0u8; 8192];
        let mut reader = tokio::io::BufReader::new(pty_err);
        loop {
            buf.clear();
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(_) => {
                    if err_tx.send(buf.clone()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Merge stdout + stderr into one read handle
    let read_handle = tokio::spawn(async move {
        tokio::select! {
            _ = rh_out => {},
            _ = rh_err => {},
        }
    });

    // Write to stdin from input channel
    let write_handle = tokio::spawn(async move {
        while let Some(data) = input_rx.recv().await {
            if (&mut pty_input).write_all(&data).await.is_err() {
                break;
            }
            let _ = (&mut pty_input).flush().await;
        }
    });

    // Forward output channel → WebSocket sender
    let ws_write_handle = tokio::spawn(async move {
        while let Some(data) = output_rx.recv().await {
            if ws_sender.send(Message::Binary(data)).await.is_err() {
                break;
            }
        }
    });

    // Read from WebSocket receiver → input channel (raw data, no \r\n conversion)
    let ws_read_handle = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Text(text) => {
                    let trimmed = text.trim();
                    if trimmed == "exit" {
                        break;
                    }
                    if trimmed.starts_with('{') {
                        continue;
                    }
                    let _ = input_tx.send(text.into_bytes());
                }
                Message::Binary(data) => {
                    let _ = input_tx.send(data);
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // ── Wait for any task to signal termination ────────────────────
    tokio::select! {
        _ = read_handle => { debug!("PTY read task finished (process exited)"); }
        _ = ws_read_handle => { debug!("PTY ws_read task finished (client disconnected)"); }
        _ = ws_write_handle => { debug!("PTY ws_write task finished (send error)"); }
        _ = write_handle => { debug!("PTY write task finished"); }
    }

    // Close output channel so remaining tasks exit cleanly
    drop(output_tx);

    debug!("PTY connection closed");
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
