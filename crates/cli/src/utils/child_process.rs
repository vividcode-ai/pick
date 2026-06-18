//! Child process utilities

use std::process::{Command as StdCommand, Output, Stdio};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Child;

const EXIT_STDIO_GRACE_MS: u64 = 100;

/// Cross-platform process spawning (async, returns child with piped stdio)
pub fn spawn_process(command: &str, args: &[String]) -> std::io::Result<std::process::Child> {
    let mut cmd = StdCommand::new(command);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.spawn()
}

/// Cross-platform process spawning via tokio (async)
pub async fn spawn_process_async(command: &str, args: &[String]) -> std::io::Result<Child> {
    let mut cmd = tokio::process::Command::new(command);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.spawn()
}

/// Synchronous process spawn + wait for output
pub fn spawn_process_sync(command: &str, args: &[String]) -> std::io::Result<Output> {
    let mut cmd = StdCommand::new(command);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.output()
}

/// Wait for a child process to terminate, draining stdio handles
/// with a grace period to avoid hangs due to inherited pipe handles.
pub async fn wait_for_child_process(child: &mut Child) -> std::io::Result<Option<i32>> {
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Drain stdout/stderr in background
    let drain_stdout = async move {
        if let Some(mut s) = stdout {
            let mut buf = Vec::new();
            let _ = s.read_to_end(&mut buf).await;
        }
    };
    let drain_stderr = async move {
        if let Some(mut s) = stderr {
            let mut buf = Vec::new();
            let _ = s.read_to_end(&mut buf).await;
        }
    };

    let drain = tokio::spawn(async move {
        tokio::join!(drain_stdout, drain_stderr);
    });

    // Wait for process exit with grace period for stdio to drain
    let exit_status =
        tokio::time::timeout(Duration::from_millis(EXIT_STDIO_GRACE_MS), child.wait()).await;

    // Ensure drain completes
    let _ = drain.await;

    match exit_status {
        Ok(Ok(status)) => Ok(status.code()),
        Ok(Err(e)) => Err(e),
        Err(_) => {
            // Grace period expired — return the exit code if process already exited
            match child.try_wait() {
                Ok(Some(status)) => Ok(status.code()),
                Ok(None) => Ok(None),
                Err(e) => Err(e),
            }
        }
    }
}
