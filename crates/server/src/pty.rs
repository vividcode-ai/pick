use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
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

// ─── ConPTY: Windows pseudo console ─────────────────────────────────────────
#[cfg(target_os = "windows")]
mod conpty {
    use std::ffi::OsStr;
    use std::io::{self};
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::FromRawHandle;
    use std::ptr;

    type HANDLE = isize;
    type BOOL = i32;
    type DWORD = u32;
    type HRESULT = i32;
    type LPVOID = *mut std::ffi::c_void;
    type LPCWSTR = *const u16;
    type LPWSTR = *mut u16;

    const FALSE: BOOL = 0;
    const TRUE: BOOL = 1;
    const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: DWORD = 0x00020016;
    const EXTENDED_STARTUPINFO_PRESENT: DWORD = 0x00080000;
    const HANDLE_FLAG_INHERIT: DWORD = 1;

    #[repr(C)]
    struct COORD {
        x: i16,
        y: i16,
    }

    #[repr(C)]
    struct SECURITY_ATTRIBUTES {
        n_length: DWORD,
        lp_security_descriptor: LPVOID,
        b_inherit_handle: BOOL,
    }

    #[repr(C)]
    struct STARTUPINFOW {
        cb: DWORD,
        lp_reserved: LPWSTR,
        lp_desktop: LPWSTR,
        lp_title: LPWSTR,
        dw_x: DWORD,
        dw_y: DWORD,
        dw_x_size: DWORD,
        dw_y_size: DWORD,
        dw_x_count_chars: DWORD,
        dw_y_count_chars: DWORD,
        dw_fill_attribute: DWORD,
        dw_flags: DWORD,
        w_show_window: u16,
        cb_reserved2: u16,
        lp_reserved2: *mut u8,
        h_std_input: HANDLE,
        h_std_output: HANDLE,
        h_std_error: HANDLE,
    }

    #[repr(C)]
    struct PROCESS_INFORMATION {
        h_process: HANDLE,
        h_thread: HANDLE,
        dw_process_id: DWORD,
        dw_thread_id: DWORD,
    }

    #[repr(C)]
    struct STARTUPINFOEXW {
        startup_info: STARTUPINFOW,
        lp_attribute_list: *mut u8,
    }

    unsafe extern "system" {
        fn CreatePipe(
            ph_read: *mut HANDLE,
            ph_write: *mut HANDLE,
            lp_pipe_attributes: *const SECURITY_ATTRIBUTES,
            n_size: DWORD,
        ) -> BOOL;

        fn CloseHandle(h_object: HANDLE) -> BOOL;

        fn SetHandleInformation(h_object: HANDLE, dw_mask: DWORD, dw_flags: DWORD) -> BOOL;

        fn CreatePseudoConsole(
            size: COORD,
            h_input: HANDLE,
            h_output: HANDLE,
            dw_flags: DWORD,
            ph_console: *mut HANDLE,
        ) -> HRESULT;

        fn ResizePseudoConsole(h_console: HANDLE, size: COORD) -> HRESULT;

        fn ClosePseudoConsole(h_console: HANDLE);

        fn InitializeProcThreadAttributeList(
            lp_attribute_list: *mut u8,
            dw_attribute_count: DWORD,
            dw_flags: DWORD,
            lp_size: *mut usize,
        ) -> BOOL;

        fn UpdateProcThreadAttribute(
            lp_attribute_list: *mut u8,
            dw_flags: DWORD,
            attribute: usize,
            lp_value: LPVOID,
            cb_size: usize,
            lp_previous_value: LPVOID,
            lp_return_size: *mut usize,
        ) -> BOOL;

        fn DeleteProcThreadAttributeList(lp_attribute_list: *mut u8) -> BOOL;

        fn CreateProcessW(
            lp_application_name: LPCWSTR,
            lp_command_line: LPWSTR,
            lp_process_attributes: *const SECURITY_ATTRIBUTES,
            lp_thread_attributes: *const SECURITY_ATTRIBUTES,
            b_inherit_handles: BOOL,
            dw_creation_flags: DWORD,
            lp_environment: LPVOID,
            lp_current_directory: LPCWSTR,
            lp_startup_info: *const STARTUPINFOW,
            lp_process_information: *mut PROCESS_INFORMATION,
        ) -> BOOL;
    }

    fn to_utf16(s: &str) -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    /// Windows ConPTY wrapper.
    /// On drop, `ClosePseudoConsole` is called, which terminates the child process.
    pub struct WinConPty {
        h_pc: HANDLE,
        read_file: std::fs::File,
        write_file: std::fs::File,
        rows: u16,
        cols: u16,
    }

    impl WinConPty {
        /// Spawn shell with ConPTY.  Returns the `WinConPty` whose `Drop`
        /// calls `ClosePseudoConsole` (terminating the child).
        /// The child process handle is closed inside – we don't need it
        /// since `ClosePseudoConsole` manages child lifetime.
        pub fn spawn(shell: &str, cwd: Option<&str>, rows: u16, cols: u16) -> io::Result<Self> {
            unsafe {
                // ── 1. Create PTY pipes ──────────────────────────────────
                let sa = SECURITY_ATTRIBUTES {
                    n_length: std::mem::size_of::<SECURITY_ATTRIBUTES>() as DWORD,
                    lp_security_descriptor: ptr::null_mut(),
                    b_inherit_handle: TRUE,
                };

                let mut in_read: HANDLE = 0;
                let mut in_write: HANDLE = 0;
                let mut out_read: HANDLE = 0;
                let mut out_write: HANDLE = 0;

                if CreatePipe(&mut in_read, &mut in_write, &sa, 0) == FALSE {
                    return Err(io::Error::last_os_error());
                }
                if CreatePipe(&mut out_read, &mut out_write, &sa, 0) == FALSE {
                    let _ = CloseHandle(in_read);
                    let _ = CloseHandle(in_write);
                    return Err(io::Error::last_os_error());
                }

                // ── 2. Create pseudo console ─────────────────────────────
                let size = COORD {
                    x: cols as i16,
                    y: rows as i16,
                };
                let mut h_pc: HANDLE = 0;
                let hr = CreatePseudoConsole(size, in_read, out_write, 0, &mut h_pc);
                if hr < 0 {
                    let _ = CloseHandle(in_read);
                    let _ = CloseHandle(in_write);
                    let _ = CloseHandle(out_read);
                    let _ = CloseHandle(out_write);
                    return Err(io::Error::from_raw_os_error(hr));
                }

                // PC owns in_read and out_write now – close our references
                CloseHandle(in_read);
                CloseHandle(out_write);

                // Disable inheritance on our I/O ends so the child doesn't inherit them
                SetHandleInformation(in_write, HANDLE_FLAG_INHERIT, 0);
                SetHandleInformation(out_read, HANDLE_FLAG_INHERIT, 0);

                // ── 3. Prepare STARTUPINFOEX with ConPTY attribute ─────
                let mut attr_list_size: usize = 0;
                InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut attr_list_size);

                let mut attr_list: Vec<u8> = vec![0u8; attr_list_size];
                if InitializeProcThreadAttributeList(
                    attr_list.as_mut_ptr(),
                    1,
                    0,
                    &mut attr_list_size,
                ) == FALSE
                {
                    let _ = CloseHandle(in_write);
                    let _ = CloseHandle(out_read);
                    ClosePseudoConsole(h_pc);
                    return Err(io::Error::last_os_error());
                }

                if UpdateProcThreadAttribute(
                    attr_list.as_mut_ptr(),
                    0,
                    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                    &mut h_pc as *mut _ as LPVOID,
                    std::mem::size_of::<HANDLE>(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                ) == FALSE
                {
                    let _ = CloseHandle(in_write);
                    let _ = CloseHandle(out_read);
                    DeleteProcThreadAttributeList(attr_list.as_mut_ptr());
                    ClosePseudoConsole(h_pc);
                    return Err(io::Error::last_os_error());
                }

                let mut si: STARTUPINFOEXW = std::mem::zeroed();
                si.startup_info.cb = std::mem::size_of::<STARTUPINFOEXW>() as DWORD;
                si.lp_attribute_list = attr_list.as_mut_ptr();

                // ── 4. Create process ────────────────────────────────────
                let shell_wide = to_utf16(shell);
                let mut pi: PROCESS_INFORMATION = std::mem::zeroed();

                let cwd_wide = cwd.map(to_utf16);

                let dir_ptr: LPCWSTR = match &cwd_wide {
                    Some(v) => v.as_ptr(),
                    None => ptr::null(),
                };

                let mut cmd_line = shell_wide.clone(); // CreateProcessW may modify this

                if CreateProcessW(
                    ptr::null(),
                    cmd_line.as_mut_ptr(),
                    ptr::null(),
                    ptr::null(),
                    TRUE,
                    EXTENDED_STARTUPINFO_PRESENT,
                    ptr::null_mut(),
                    dir_ptr,
                    &si.startup_info as *const STARTUPINFOW,
                    &mut pi,
                ) == FALSE
                {
                    let err = io::Error::last_os_error();
                    let _ = CloseHandle(in_write);
                    let _ = CloseHandle(out_read);
                    DeleteProcThreadAttributeList(attr_list.as_mut_ptr());
                    ClosePseudoConsole(h_pc);
                    return Err(err);
                }

                // Close process/thread handles – we don't need them (PC manages lifetime)
                CloseHandle(pi.h_process);
                CloseHandle(pi.h_thread);

                // Create std::fs::File wraps for our I/O pipe ends
                let read_file = std::fs::File::from_raw_handle(out_read as _);
                let write_file = std::fs::File::from_raw_handle(in_write as _);

                // The attr_list must stay alive until CreateProcessW returns.
                // After CreateProcessW returns, we can clean it up.
                DeleteProcThreadAttributeList(attr_list.as_mut_ptr());

                Ok(Self {
                    h_pc,
                    read_file,
                    write_file,
                    rows,
                    cols,
                })
            }
        }

        pub fn resize(&mut self, rows: u16, cols: u16) -> io::Result<()> {
            self.rows = rows;
            self.cols = cols;
            unsafe {
                let size = COORD {
                    x: cols as i16,
                    y: rows as i16,
                };
                let hr = ResizePseudoConsole(self.h_pc, size);
                if hr >= 0 {
                    Ok(())
                } else {
                    Err(io::Error::from_raw_os_error(hr))
                }
            }
        }

        pub fn try_clone_reader(&self) -> io::Result<std::fs::File> {
            self.read_file.try_clone()
        }

        pub fn try_clone_writer(&self) -> io::Result<std::fs::File> {
            self.write_file.try_clone()
        }
    }

    impl Drop for WinConPty {
        fn drop(&mut self) {
            unsafe {
                ClosePseudoConsole(self.h_pc);
            }
        }
    }
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

    #[cfg(target_os = "windows")]
    let pty = match tokio::task::spawn_blocking({
        let shell = shell.to_string();
        let cwd = cwd.clone();
        move || conpty::WinConPty::spawn(&shell, cwd.as_deref(), 24, 80)
    })
    .await
    {
        Ok(Ok(pty_inner)) => std::sync::Arc::new(std::sync::Mutex::new(pty_inner)),
        Ok(Err(e)) => {
            let msg = format!("\r\n\x1b[31m[PTY Error: {}]\x1b[0m\r\n", e);
            let _ = ws_sender.send(Message::Text(msg)).await;
            return Err(e.into());
        }
        Err(e) => {
            let msg = format!("\r\n\x1b[31m[PTY Error: {}]\x1b[0m\r\n", e);
            let _ = ws_sender.send(Message::Text(msg)).await;
            return Err(e.into());
        }
    };

    #[cfg(not(target_os = "windows"))]
    let (pty_input, pty_output, pty_err, _child) = {
        let mut cmd = std::process::Command::new(shell);
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        if let Some(ref cwd) = cwd {
            cmd.current_dir(cwd);
        }
        let child = tokio::task::spawn_blocking(move || cmd.spawn()).await??;
        (
            child.stdin.take().unwrap(),
            child.stdout.take().unwrap(),
            child.stderr.take().unwrap(),
            child,
        )
    };

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

    #[cfg(target_os = "windows")]
    let (read_handle, write_handle) = {
        let pty_guard = pty.lock().unwrap();
        let pty_reader = pty_guard.try_clone_reader()?;
        let pty_writer = pty_guard.try_clone_writer()?;
        drop(pty_guard);

        let read_tx = output_tx.clone();
        let rh = tokio::task::spawn_blocking(move || {
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

        let wh = tokio::task::spawn_blocking(move || {
            let mut writer = pty_writer;
            while let Some(data) = input_rx.blocking_recv() {
                if writer.write_all(&data).is_err() {
                    break;
                }
                let _ = writer.flush();
            }
        });

        (rh, wh)
    };

    #[cfg(not(target_os = "windows"))]
    let (read_handle, write_handle) = {
        // Unix: merge stdout + stderr into a single output channel
        let read_tx = output_tx.clone();
        let rh_out = tokio::spawn({
            let tx = read_tx.clone();
            async move {
                use tokio::io::AsyncReadExt;
                let mut buf = vec![0u8; 8192];
                let mut reader = tokio::io::BufReader::new(pty_output);
                loop {
                    buf.clear();
                    match reader.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(_) => {
                            if tx.send(buf.clone()).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        });
        let rh_err = tokio::spawn({
            async move {
                use tokio::io::AsyncReadExt;
                let mut buf = vec![0u8; 8192];
                let mut reader = tokio::io::BufReader::new(pty_err);
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
            }
        });

        let wh = tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let mut writer = pty_input;
            while let Some(data) = input_rx.recv().await {
                if writer.write_all(&data).await.is_err() {
                    break;
                }
                let _ = writer.flush().await;
            }
        });

        // Wrap all handles into a single read handle that completes when either task does
        let rh = tokio::spawn(async move {
            tokio::select! {
                _ = rh_out => {},
                _ = rh_err => {},
            }
        });

        (rh, wh)
    };

    // Task: forward output channel → WebSocket sender
    let ws_write_handle = tokio::spawn(async move {
        while let Some(data) = output_rx.recv().await {
            if ws_sender.send(Message::Binary(data)).await.is_err() {
                break;
            }
        }
    });

    // Task: read from WebSocket receiver → input channel
    #[cfg(target_os = "windows")]
    let ws_read_handle = {
        let pty_clone = pty.clone();
        tokio::spawn(async move {
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
                                if let Ok(mut pty_guard) = pty_clone.lock() {
                                    let _ = pty_guard.resize(rd.rows, rd.cols);
                                }
                            }
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
        })
    };

    #[cfg(not(target_os = "windows"))]
    let ws_read_handle = {
        tokio::spawn(async move {
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
        })
    };

    // ── Wait for any task to signal termination ────────────────────
    tokio::select! {
        _ = read_handle => { debug!("PTY read task finished (process exited)"); }
        _ = ws_read_handle => { debug!("PTY ws_read task finished (client disconnected)"); }
        _ = ws_write_handle => { debug!("PTY ws_write task finished (send error)"); }
        _ = write_handle => { debug!("PTY write task finished"); }
    }

    // Close output channel so ws_write_handle task will exit on its own
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
