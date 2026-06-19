//! Utility functions: shell config, process management

/// Shell configuration (path and arguments)
pub struct ShellConfig {
    pub shell: String,
    pub args: Vec<String>,
}

/// Get shell configuration for the current platform.
/// Auto-detects bash on Windows (Git Bash, PATH) and Unix (/bin/bash, PATH, sh fallback).
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
        // 1. Try Git Bash in ProgramFiles
        for b in [
            std::env::var("ProgramFiles"),
            std::env::var("ProgramFiles(x86)"),
        ].into_iter().flatten() {
            let path = format!("{}\\Git\\bin\\bash.exe", b);
            if std::path::Path::new(&path).exists() {
                return Ok(ShellConfig {
                    shell: path,
                    args: vec!["-c".to_string()],
                });
            }
        }
        // 2. Scan drive roots for Git\bin\bash.exe (catches D:\Git, C:\Git, etc.)
        for drive in 'C'..='Z' {
            let path = format!("{}:\\Git\\bin\\bash.exe", drive);
            if std::path::Path::new(&path).exists() {
                return Ok(ShellConfig {
                    shell: path,
                    args: vec!["-c".to_string()],
                });
            }
        }
        // 3. Search bash.exe on PATH (Cygwin, MSYS2, WSL)
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
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
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
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
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
