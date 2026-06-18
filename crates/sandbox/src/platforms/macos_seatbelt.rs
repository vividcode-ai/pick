use std::path::Path;

use pick_agent::permission::sandbox::{Sandbox, SandboxConfig, SandboxRequest, SandboxType};

pub struct MacosSeatbeltSandbox;

impl MacosSeatbeltSandbox {
    pub fn new(_config: &SandboxConfig) -> Self {
        Self
    }

    fn check_sandbox_exec() -> bool {
        Path::new("/usr/bin/sandbox-exec").exists()
    }

    fn build_seatbelt_policy(req: &SandboxRequest) -> String {
        let mut policy = String::from("(version 1)\n(deny default)\n");

        policy.push_str("(allow process-exec)\n");
        policy.push_str("(allow process-fork)\n");
        policy.push_str("(allow signal)\n");
        policy.push_str("(allow sysctl-read)\n");

        for path in &[
            "/usr/lib",
            "/usr/share",
            "/bin",
            "/sbin",
            "/usr/bin",
            "/usr/sbin",
        ] {
            policy.push_str(&format!("(allow file-read* (subpath \"{}\"))\n", path));
        }
        policy.push_str("(allow file-read* (subpath \"/tmp\"))\n");
        policy.push_str("(allow file-write* (subpath \"/tmp\"))\n");
        policy.push_str("(allow file-read* (subpath \"/private/tmp\"))\n");
        policy.push_str("(allow file-write* (subpath \"/private/tmp\"))\n");

        policy.push_str("(allow file-read* (literal \"/dev/null\"))\n");
        policy.push_str("(allow file-write* (literal \"/dev/null\"))\n");

        if let Some(ref fs) = req.fs_policy {
            for root in &fs.readable_roots {
                let r = root.to_string_lossy();
                policy.push_str(&format!("(allow file-read* (subpath \"{}\"))\n", r));
            }

            for root in &fs.writable_roots {
                let r = root.path.to_string_lossy();
                policy.push_str(&format!("(allow file-read* (subpath \"{}\"))\n", r));
                let mut write_rule = format!("(allow file-write* (require-all (subpath \"{}\")", r);
                for prot in &fs.protected_paths {
                    let p = prot.trim_end_matches("/**");
                    write_rule.push_str(&format!(" (require-not (subpath \"{}\"))", p));
                }
                write_rule.push_str("))\n");
                policy.push_str(&write_rule);
            }
        }

        policy.push_str(&format!(
            "(allow file-read* (subpath \"{}\"))\n",
            req.cwd.to_string_lossy()
        ));

        policy
    }
}

impl Sandbox for MacosSeatbeltSandbox {
    fn sandbox_type(&self) -> SandboxType {
        SandboxType::MacosSeatbelt
    }

    fn name(&self) -> &str {
        "macos-seatbelt"
    }

    fn is_available(&self) -> bool {
        Self::check_sandbox_exec()
    }

    fn transform(&self, req: &SandboxRequest) -> Result<(String, Vec<String>), String> {
        let policy = Self::build_seatbelt_policy(req);

        let mut args = vec![
            "-p".to_string(),
            policy,
            "--".to_string(),
            req.program.clone(),
        ];
        args.extend(req.args.clone());

        Ok(("sandbox-exec".into(), args))
    }
}
