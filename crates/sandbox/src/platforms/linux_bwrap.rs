use std::path::Path;

use pick_agent::permission::fs_policy::AccessMode;
use pick_agent::permission::sandbox::{SandboxConfig, SandboxRequest, Sandbox, SandboxType};
use pick_agent::permission::profiles::NetworkAccess;

pub struct LinuxBwrapSandbox;

impl LinuxBwrapSandbox {
    pub fn new(_config: &SandboxConfig) -> Self {
        Self
    }

    fn check_bwrap() -> bool {
        std::process::Command::new("which")
            .arg("bwrap")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

impl Sandbox for LinuxBwrapSandbox {
    fn sandbox_type(&self) -> SandboxType {
        SandboxType::LinuxBwrap
    }

    fn name(&self) -> &str {
        "linux-bwrap"
    }

    fn is_available(&self) -> bool {
        Self::check_bwrap()
    }

    fn transform(
        &self,
        req: &SandboxRequest,
    ) -> Result<(String, Vec<String>), String> {
        let policy = match req.fs_policy {
            Some(ref p) => p,
            None => return Ok((req.program.clone(), req.args.clone())),
        };

        let mut bwrap_args: Vec<String> = Vec::new();

        // Essential isolation flags
        bwrap_args.push("--new-session".into());
        bwrap_args.push("--die-with-parent".into());
        bwrap_args.push("--unshare-user".into());
        bwrap_args.push("--unshare-pid".into());
        bwrap_args.push("--dev".into());
        bwrap_args.push("/dev".into());
        bwrap_args.push("--proc".into());
        bwrap_args.push("/proc".into());

        // Network isolation: block network when not explicitly set to Full
        if req.network_access != NetworkAccess::Full {
            bwrap_args.push("--unshare-net".into());
        }

        // Default: read-only root filesystem
        bwrap_args.push("--ro-bind".into());
        bwrap_args.push("/".into());
        bwrap_args.push("/".into());

        // Writable roots: bind-mount with write access
        for root in &policy.writable_roots {
            let root_str = root.path.to_string_lossy().to_string();
            if root.mode == AccessMode::Write {
                bwrap_args.push("--bind".into());
            } else {
                bwrap_args.push("--ro-bind".into());
            }
            bwrap_args.push(root_str.clone());
            bwrap_args.push(root_str);
        }

        // Separate readable roots (read-only)
        for root in &policy.readable_roots {
            let root_str = root.to_string_lossy().to_string();
            bwrap_args.push("--ro-bind".into());
            bwrap_args.push(root_str.clone());
            bwrap_args.push(root_str);
        }

        // Protected metadata paths: re-mount as read-only on top of writable roots
        for pattern in &policy.protected_paths {
            let p = pattern.trim_end_matches("/**");
            if !p.contains('*') && Path::new(p).exists() {
                bwrap_args.push("--ro-bind".into());
                bwrap_args.push(p.to_string());
                bwrap_args.push(p.to_string());
            }
        }

        // Working directory
        bwrap_args.push("--chdir".into());
        bwrap_args.push(req.cwd.to_string_lossy().to_string());

        // The actual command
        bwrap_args.push("--".into());
        bwrap_args.push(req.program.clone());
        bwrap_args.extend(req.args.clone());

        Ok(("bwrap".into(), bwrap_args))
    }
}
