use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::fs_policy::FileSystemPolicy;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SandboxType {
    None,
    WindowsJob,
    LinuxBwrap,
    MacosSeatbelt,
}

#[derive(Debug, Clone)]
pub struct SandboxConfig {
    pub sandbox_type: SandboxType,
    pub read_write_paths: Vec<PathBuf>,
    pub read_only_paths: Vec<PathBuf>,
    pub network_access: super::profiles::NetworkAccess,
    pub timeout_secs: u64,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            sandbox_type: SandboxType::None,
            read_write_paths: Vec::new(),
            read_only_paths: Vec::new(),
            network_access: super::profiles::NetworkAccess::Restricted,
            timeout_secs: 120,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SandboxRequest {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
    pub fs_policy: Option<Arc<FileSystemPolicy>>,
    pub timeout_secs: u64,
    pub network_access: super::profiles::NetworkAccess,
}

impl SandboxRequest {
    pub fn new(
        program: &str,
        args: &[String],
        cwd: &Path,
        fs_policy: Option<Arc<FileSystemPolicy>>,
        timeout_secs: u64,
    ) -> Self {
        Self {
            program: program.to_string(),
            args: args.to_vec(),
            cwd: cwd.to_path_buf(),
            env: HashMap::new(),
            fs_policy,
            timeout_secs,
            network_access: super::profiles::NetworkAccess::Full,
        }
    }
}

pub trait Sandbox: Send + Sync {
    fn sandbox_type(&self) -> SandboxType;
    fn name(&self) -> &str;
    fn is_available(&self) -> bool {
        true
    }
    /// Transform the command: return (program, args) wrapped in sandbox.
    /// For platforms where sandbox wraps via argv (Linux bwrap, macOS seatbelt).
    fn transform(&self, req: &SandboxRequest) -> Result<(String, Vec<String>), String>;
    /// Apply sandbox at spawn time (Windows restricted token, job objects).
    /// This modifies the command's creation flags and env vars before spawn.
    fn spawn(&self, cmd: &mut std::process::Command, req: &SandboxRequest) -> Result<(), String> {
        let _ = (cmd, req);
        Ok(())
    }
    /// Directly spawn a command under sandbox control, returning (exit_code, stdout, stderr).
    /// Default impl returns None (use transform+spawn instead).
    /// Windows overrides this to use CreateProcessAsUserW with restricted token.
    fn direct_spawn(
        &self,
        _command: &str,
        _req: &SandboxRequest,
    ) -> Option<Result<(i32, String, String), String>> {
        None
    }
    fn is_windows_sandbox(&self) -> bool {
        false
    }
}

pub struct NoSandbox;

impl Sandbox for NoSandbox {
    fn sandbox_type(&self) -> SandboxType {
        SandboxType::None
    }

    fn name(&self) -> &str {
        "none"
    }

    fn is_available(&self) -> bool {
        false
    }

    fn transform(&self, _req: &SandboxRequest) -> Result<(String, Vec<String>), String> {
        Err("No sandbox available".into())
    }
}

pub struct WindowsJobSandbox;

impl WindowsJobSandbox {
    #[cfg(windows)]
    fn apply_job_object(cmd: &mut std::process::Command) -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        // CREATE_BREAKAWAY_FROM_JOB | CREATE_NEW_PROCESS_GROUP | CREATE_SUSPENDED
        const CREATE_FLAGS: u32 = 0x02000000 | 0x00000200 | 0x00000004;
        cmd.creation_flags(CREATE_FLAGS);
        Ok(())
    }

    #[cfg(not(windows))]
    fn apply_job_object(_cmd: &mut std::process::Command) -> Result<(), String> {
        Ok(())
    }
}

impl Sandbox for WindowsJobSandbox {
    fn sandbox_type(&self) -> SandboxType {
        SandboxType::WindowsJob
    }

    fn name(&self) -> &str {
        "windows-job"
    }

    fn is_windows_sandbox(&self) -> bool {
        true
    }

    fn transform(&self, req: &SandboxRequest) -> Result<(String, Vec<String>), String> {
        Ok((req.program.clone(), req.args.clone()))
    }

    fn spawn(&self, cmd: &mut std::process::Command, req: &SandboxRequest) -> Result<(), String> {
        Self::apply_job_object(cmd)?;

        if let Some(ref fp) = req.fs_policy {
            let writable: Vec<String> = fp
                .writable_roots
                .iter()
                .map(|r| r.path.to_string_lossy().to_string())
                .collect();
            if !writable.is_empty() {
                cmd.env("pick_SANDBOX_WRITABLE", writable.join(";"));
            }
        }

        if req.timeout_secs > 0 {
            cmd.env("pick_SANDBOX_TIMEOUT", req.timeout_secs.to_string());
        }

        Ok(())
    }
}

pub fn create_sandbox(sandbox_type: SandboxType) -> Box<dyn Sandbox> {
    match sandbox_type {
        SandboxType::None => Box::new(NoSandbox),
        SandboxType::WindowsJob => Box::new(WindowsJobSandbox),
        _ => Box::new(NoSandbox),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_creation() {
        let s = create_sandbox(SandboxType::None);
        assert_eq!(s.sandbox_type(), SandboxType::None);

        let s = create_sandbox(SandboxType::WindowsJob);
        assert_eq!(s.sandbox_type(), SandboxType::WindowsJob);
    }

    #[test]
    fn test_no_sandbox_not_available() {
        let s = NoSandbox;
        assert!(!s.is_available());
        assert_eq!(s.name(), "none");
        assert_eq!(s.sandbox_type(), SandboxType::None);
    }

    #[test]
    fn test_sandbox_config_default() {
        let config = SandboxConfig::default();
        assert_eq!(config.timeout_secs, 120);
        assert_eq!(config.sandbox_type, SandboxType::None);
    }
}
