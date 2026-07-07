use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use tokio::sync::{Mutex, RwLock};
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
        let shell = detect_shell();
        let child = match spawn_shell_piped(&shell, self.cwd.as_deref()) {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to spawn shell {}: {}", shell, e);
                return String::new();
            }
        };

        let session = PtySession {
            child: Arc::new(Mutex::new(child)),
            created_at: now,
            shell: shell.clone(),
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
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd.spawn()
}

/// Shells that should not be auto-selected as default.
#[allow(dead_code)]
const SHELL_BLACKLIST: &[&str] = &["fish", "nu"];

/// Extract the lowercase file stem from a shell path (e.g. "/usr/bin/fish" → "fish").
#[allow(dead_code)]
fn shell_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default()
}

/// Find an executable in PATH, like the `which` command.
fn which(cmd: &str) -> Option<String> {
    let exe = if cfg!(target_os = "windows") {
        if cmd.ends_with(".exe") {
            cmd.to_string()
        } else {
            format!("{}.exe", cmd)
        }
    } else {
        cmd.to_string()
    };
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let full = dir.join(&exe);
            if full.is_file() {
                Some(full.to_string_lossy().into_owned())
            } else {
                None
            }
        })
    })
}

/// On Windows, detect Git-bundled bash.exe via git --exec-path.
fn git_bash_path() -> Option<String> {
    let git = which("git.exe")?;
    let git_dir = Path::new(&git).parent()?;
    let bash = git_dir.parent()?.join("bin").join("bash.exe");
    if bash.is_file() {
        Some(bash.to_string_lossy().into_owned())
    } else {
        None
    }
}

/// Detect the best available shell, matching OpenCode's logic.
fn detect_shell() -> String {
    // Check $SHELL first on non-Windows (skip blacklisted shells)
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(shell) = std::env::var("SHELL") {
            let name = shell_name(&shell);
            if !shell.is_empty() && !SHELL_BLACKLIST.contains(&name.as_str()) {
                return shell;
            }
        }
        if let Some(path) = which("bash") {
            return path;
        }
        #[cfg(target_os = "macos")]
        {
            if Path::new("/bin/zsh").is_file() {
                return "/bin/zsh".to_string();
            }
        }
        return "/bin/sh".to_string();
    }

    // Windows fallback chain
    #[cfg(target_os = "windows")]
    {
        // 1. PowerShell Core
        if let Some(path) = which("pwsh.exe") {
            return path;
        }
        // 2. Windows PowerShell
        if let Some(path) = which("powershell.exe") {
            return path;
        }
        // 3. Git Bash
        if let Some(path) = git_bash_path() {
            return path;
        }
        // 4. COMSPEC or cmd.exe
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
    }
}

/// Axum handler: upgrade to WebSocket and run a PTY session.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<crate::AppState>>,
) -> impl IntoResponse {
    let cwd = state.pty_manager.cwd().map(|s| s.to_string());
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_pty_connection(socket, cwd).await {
            warn!("PTY session error: {}", e);
        }
    })
}

async fn handle_pty_connection(
    socket: WebSocket,
    cwd: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    let shell = detect_shell();

    // Spawn shell with portable-pty (ConPTY on Windows, forkpty on Unix)
    let pair_result = tokio::task::spawn_blocking({
        let shell = shell.to_string();
        let cwd = cwd.clone();
        move || {
            let pty_system = NativePtySystem::default();
            let size = PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            };
            let pair = pty_system.openpty(size)?;
            let mut cmd = CommandBuilder::new(&shell);
            if let Some(ref cwd) = cwd {
                cmd.cwd(cwd);
            }
            cmd.env("TERM", "xterm-256color");
            let child = pair.slave.spawn_command(cmd)?;
            Ok::<_, anyhow::Error>((pair.master, child))
        }
    })
    .await;

    let (master_pty, _child_pty) = match pair_result {
        Ok(Ok(pair)) => pair,
        Ok(Err(e)) => {
            return Err(std::io::Error::other(format!("PTY: {}", e)).into());
        }
        Err(e) => {
            return Err(std::io::Error::other(format!("PTY join: {}", e)).into());
        }
    };

    // Welcome message
    let welcome = format!(
        "\r\n*** Pick Terminal ({}): Type 'exit' to close ***\r\n",
        shell
    );
    let _ = ws_sender.send(Message::Text(welcome.into())).await;

    // ── Set up I/O channels ──────────────────────────────────────────
    let (output_tx, mut output_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let (input_tx, mut input_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

    // ── Clone PTY reader/writer and share master for resize ──────────
    let pty_reader = master_pty.try_clone_reader()?;
    let pty_writer = master_pty.take_writer()?;
    let master = Arc::new(std::sync::Mutex::new(master_pty));

    // ── I/O tasks ────────────────────────────────────────────────────

    // Read from PTY output → output channel (blocking I/O)
    let read_tx = output_tx.clone();
    let read_handle = tokio::task::spawn_blocking(move || {
        let mut buf = vec![0u8; 8192];
        let mut reader = pty_reader;
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if read_tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Write to PTY input from input channel (blocking I/O)
    let write_handle = tokio::task::spawn_blocking(move || {
        let mut writer = pty_writer;
        while let Some(data) = input_rx.blocking_recv() {
            if writer.write_all(&data).is_err() {
                break;
            }
            let _ = writer.flush();
        }
    });

    // Forward output channel → WebSocket sender
    let ws_write_handle = tokio::spawn(async move {
        while let Some(data) = output_rx.recv().await {
            if ws_sender.send(Message::Binary(data.into())).await.is_err() {
                break;
            }
        }
    });

    // Read from WebSocket receiver → input channel (with resize handling)
    let master_for_resize = master.clone();
    let ws_read_handle = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Text(text) => {
                    let trimmed = text.trim();
                    if trimmed == "exit" {
                        break;
                    }
                    if trimmed.starts_with('{') {
                        #[derive(serde::Deserialize)]
                        struct ResizeData {
                            #[serde(rename = "type")]
                            _type: String,
                            cols: u16,
                            rows: u16,
                        }
                        if let Ok(rd) = serde_json::from_str::<ResizeData>(trimmed) {
                            let size = PtySize {
                                rows: rd.rows,
                                cols: rd.cols,
                                pixel_width: 0,
                                pixel_height: 0,
                            };
                            if let Ok(m) = master_for_resize.lock() {
                                let _ = m.resize(size);
                            }
                        }
                        continue;
                    }
                    let _ = input_tx.send(text.to_string().into_bytes());
                }
                Message::Binary(data) => {
                    let _ = input_tx.send(data.to_vec());
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
