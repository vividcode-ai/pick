//! Shell utilities

use std::collections::HashSet;
use std::process::Stdio;
use std::sync::LazyLock;
use std::sync::Mutex;

/// Shell configuration (path and arguments)
pub struct ShellConfig {
    pub shell: String,
    pub args: Vec<String>,
}

/// Get shell configuration for the current platform
pub fn get_shell_config(custom_shell_path: Option<&str>) -> Result<ShellConfig, String> {
    if let Some(path) = custom_shell_path {
        if std::path::Path::new(path).exists() {
            return Ok(ShellConfig {
                shell: path.to_string(),
                args: vec!["-c".to_string()],
            });
        }
        return Err(format!("Custom shell path not found: {}", path));
    }

    #[cfg(windows)]
    {
        // Try Git Bash
        for b in [
            std::env::var("ProgramFiles"),
            std::env::var("ProgramFiles(x86)"),
        ]
        .into_iter()
        .flatten()
        {
            let path = format!("{}\\Git\\bin\\bash.exe", b);
            if std::path::Path::new(&path).exists() {
                return Ok(ShellConfig {
                    shell: path,
                    args: vec!["-c".to_string()],
                });
            }
        }
        // Try PATH
        if let Some(path) = find_bash_on_path() {
            return Ok(ShellConfig {
                shell: path,
                args: vec!["-c".to_string()],
            });
        }
        Err(
            "No bash shell found. Install Git for Windows or set shellPath in settings."
                .to_string(),
        )
    }

    #[cfg(not(windows))]
    {
        if std::path::Path::new("/bin/bash").exists() {
            return Ok(ShellConfig {
                shell: "/bin/bash".to_string(),
                args: vec!["-c".to_string()],
            });
        }
        if let Some(path) = find_bash_on_path() {
            return Ok(ShellConfig {
                shell: path,
                args: vec!["-c".to_string()],
            });
        }
        Ok(ShellConfig {
            shell: "sh".to_string(),
            args: vec!["-c".to_string()],
        })
    }
}

#[cfg(windows)]
fn find_bash_on_path() -> Option<String> {
    use std::process::Command;
    Command::new("where")
        .args(["bash.exe"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let out = String::from_utf8_lossy(&o.stdout);
                out.lines().next().map(|s| s.trim().to_string())
            } else {
                None
            }
        })
}

#[cfg(not(windows))]
fn find_bash_on_path() -> Option<String> {
    use std::process::Command;
    Command::new("which")
        .args(["bash"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let out = String::from_utf8_lossy(&o.stdout);
                out.lines().next().map(|s| s.trim().to_string())
            } else {
                None
            }
        })
}

/// Sanitize binary output for display
pub fn sanitize_binary_output(output: &str) -> String {
    let mut result = String::with_capacity(output.len());
    for c in output.chars() {
        let code = c as u32;
        // Allow tab, newline, carriage return
        if code == 0x09 || code == 0x0a || code == 0x0d {
            result.push(c);
        // Skip control characters
        } else if code <= 0x1f {
            continue;
        // Skip Unicode format characters
        } else if (0xfff9..=0xfffb).contains(&code) {
            continue;
        } else {
            result.push(c);
        }
    }
    result
}

static TRACKED_PIDS: LazyLock<Mutex<HashSet<u32>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Track a detached child process PID
pub fn track_detached_child_pid(pid: u32) {
    if let Ok(mut pids) = TRACKED_PIDS.lock() {
        pids.insert(pid);
    }
}

/// Untrack a detached child process PID
pub fn untrack_detached_child_pid(pid: u32) {
    if let Ok(mut pids) = TRACKED_PIDS.lock() {
        pids.remove(&pid);
    }
}

/// Kill all tracked detached child processes
pub fn kill_tracked_detached_children() {
    if let Ok(mut pids) = TRACKED_PIDS.lock() {
        for &pid in pids.iter() {
            kill_process_tree(pid);
        }
        pids.clear();
    }
}

/// Kill a process and all its children (cross-platform)
pub fn kill_process_tree(pid: u32) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
    #[cfg(not(windows))]
    {
        use nix::sys::signal::{Signal, killpg};
        use nix::unistd::Pid;
        // Try process group first, then individual process
        if killpg(Pid::from_raw(pid as i32), Signal::SIGKILL).is_err() {
            let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGKILL);
        }
    }
}
