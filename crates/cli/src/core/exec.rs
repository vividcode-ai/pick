//! Shared command execution utilities for extensions and custom tools


use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// Options for executing shell commands
#[derive(Debug, Clone, Default)]
pub struct ExecOptions {
    /// Timeout in milliseconds
    pub timeout_ms: Option<u64>,
    /// Working directory
    pub cwd: Option<String>,
}

/// Result of executing a shell command
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub killed: bool,
}

/// Execute a shell command and return stdout/stderr/code.
/// Supports timeout.
pub async fn exec_command(
    command: &str,
    args: &[String],
    options: Option<&ExecOptions>,
) -> ExecResult {
    let mut cmd = Command::new(command);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    if let Some(cwd) = options.and_then(|o| o.cwd.as_ref()) {
        cmd.current_dir(cwd);
    }

    let timeout_dur = options
        .and_then(|o| o.timeout_ms)
        .map(Duration::from_millis);

    let result = if let Some(dur) = timeout_dur {
        match timeout(dur, cmd.output()).await {
            Ok(Ok(output)) => ExecResult {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code().unwrap_or(-1),
                killed: false,
            },
            Ok(Err(_)) => ExecResult {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 1,
                killed: false,
            },
            Err(_) => ExecResult {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: -1,
                killed: true,
            },
        }
    } else {
        match cmd.output().await {
            Ok(output) => ExecResult {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code().unwrap_or(-1),
                killed: false,
            },
            Err(_) => ExecResult {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 1,
                killed: false,
            },
        }
    };

    result
}

/// Execute a shell command with timeout
pub async fn exec_command_with_timeout(
    command: &str,
    args: &[String],
    timeout_ms: u64,
    cwd: Option<String>,
) -> ExecResult {
    exec_command(
        command,
        args,
        Some(&ExecOptions {
            timeout_ms: Some(timeout_ms),
            cwd,
        }),
    )
    .await
}
