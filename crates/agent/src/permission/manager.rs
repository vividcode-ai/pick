use std::path::Path;
use std::sync::Arc;

use super::approval::{ApprovalPolicy, PermissionConfig};
use super::audit::{AuditDecision, AuditLayer, AuditTrail};
use super::exec_policy::ExecPolicy;
use super::external_dir::ExternalDirectoryAuth;
use super::guardian::{Guardian, GuardianConfig};
use super::hooks::{PermissionHookRegistry, PermissionRequestHook};
use super::network::NetworkPolicy;
use super::profiles::{NetworkAccess, PermissionProfile};
use super::sandbox::{SandboxConfig, SandboxType};

pub struct PermissionManager {
    pub profile: PermissionProfile,
    pub hook_registry: Arc<PermissionHookRegistry>,
    pub exec_policy: Option<Arc<ExecPolicy>>,
    pub network_policy: Option<Arc<NetworkPolicy>>,
    pub guardian: Option<Arc<Guardian>>,
    pub sandbox_config: Option<SandboxConfig>,
    pub approval_policy: ApprovalPolicy,
    pub permission_config: Option<PermissionConfig>,
    pub external_auth: ExternalDirectoryAuth,
    pub audit_trail: Arc<AuditTrail>,
}

impl PermissionManager {
    pub fn new(
        profile_str: &str,
        workspace_root: &Path,
        config: Option<&PermissionConfig>,
        rules_files: &[String],
    ) -> Self {
        let approval_policy = config
            .map(|c| c.approval_policy)
            .unwrap_or(ApprovalPolicy::OnRequest);

        let profile = PermissionProfile::resolve(profile_str, workspace_root, config);

        let hook_registry = Arc::new(PermissionHookRegistry::new());

        let exec_policy = if profile.exec_policy_enabled {
            let mut ep = ExecPolicy::new();
            for path in rules_files {
                let p = std::path::Path::new(path);
                if p.exists()
                    && let Err(e) = ep.load_rules_from_file(p)
                {
                    eprintln!(
                        "[Pick] Warning: failed to load rules file '{}': {}",
                        p.display(),
                        e
                    );
                }
            }
            Some(Arc::new(ep))
        } else {
            None
        };

        let network_policy = if profile.network_access != NetworkAccess::Full {
            match profile.network_access {
                NetworkAccess::Blocked => Some(Arc::new(NetworkPolicy::new_blocked())),
                NetworkAccess::Restricted | NetworkAccess::ProxyOnly => {
                    Some(Arc::new(NetworkPolicy::new_restricted()))
                }
                _ => None,
            }
        } else {
            Some(Arc::new(NetworkPolicy::new_full_access()))
        };

        let guardian = if profile.guardian_enabled {
            Some(Arc::new(Guardian::new(GuardianConfig {
                enabled: true,
                model: config
                    .and(None)
                    .or_else(|| Some("claude-hy-4-20250514".to_string())),
                provider: None,
                strict_auto_review: false,
            })))
        } else {
            None
        };

        let sandbox_enabled = config.map(|c| c.sandbox_enabled).unwrap_or(true);
        let sandbox_config = if profile.sandbox_enabled && sandbox_enabled {
            let sandbox_type = if cfg!(target_os = "linux") {
                SandboxType::LinuxBwrap
            } else if cfg!(target_os = "macos") {
                SandboxType::MacosSeatbelt
            } else if cfg!(windows) {
                SandboxType::WindowsJob
            } else {
                SandboxType::None
            };
            Some(SandboxConfig {
                sandbox_type,
                read_write_paths: vec![workspace_root.to_path_buf()],
                read_only_paths: Vec::new(),
                network_access: profile.network_access.clone(),
                timeout_secs: 120,
            })
        } else {
            None
        };

        let external_auth = ExternalDirectoryAuth::load(workspace_root);

        let audit_trail = {
            let audit_path = workspace_root.join(".pick").join("audit.jsonl");
            Arc::new(AuditTrail::new().with_file(&audit_path))
        };

        Self {
            profile,
            hook_registry,
            exec_policy,
            network_policy,
            guardian,
            sandbox_config,
            approval_policy,
            permission_config: config.cloned(),
            external_auth,
            audit_trail,
        }
    }

    pub fn should_prompt(&self, is_sandbox_issue: bool) -> bool {
        match self.approval_policy {
            ApprovalPolicy::Never => false,
            ApprovalPolicy::OnRequest => true,
            ApprovalPolicy::OnFailure => is_sandbox_issue,
            ApprovalPolicy::UnlessTrusted => {
                // UnlessTrusted always returns false here — the command-aware
                // decision is handled by should_prompt_for_command.
                false
            }
            ApprovalPolicy::Granular => self
                .permission_config
                .as_ref()
                .and_then(|c| c.granular.as_ref())
                .map(|g| g.sandbox_approval)
                .unwrap_or(true),
        }
    }

    /// Command-aware approval check for UnlessTrusted policy.
    /// Returns true if the user should be prompted for this command.
    pub fn should_prompt_for_command(&self, command: &str) -> bool {
        match self.approval_policy {
            ApprovalPolicy::UnlessTrusted => {
                if let Some(ref ep) = self.exec_policy {
                    // Only prompt for commands that aren't explicitly allowed
                    matches!(
                        ep.evaluate(command),
                        super::exec_policy::ExecDecision::Prompt
                    )
                } else {
                    // No exec policy = no trust info = prompt
                    true
                }
            }
            // All other policies delegate to should_prompt
            _ => self.should_prompt(false),
        }
    }

    pub fn check_exec_policy(&self, command: &str) -> Option<String> {
        if let Some(ref ep) = self.exec_policy {
            match ep.evaluate(command) {
                super::exec_policy::ExecDecision::Forbidden => {
                    Some(format!("ExecPolicy: command '{}' is forbidden", command))
                }
                super::exec_policy::ExecDecision::Prompt => {
                    if self.approval_policy == ApprovalPolicy::Never {
                        return Some(format!(
                            "ExecPolicy: command '{}' requires approval but approval_policy is 'never'",
                            command
                        ));
                    }
                    None
                }
                super::exec_policy::ExecDecision::Allow => None,
            }
        } else {
            None
        }
    }

    pub fn check_network(&self, url: &str) -> Result<(), String> {
        if let Some(ref np) = self.network_policy {
            np.can_access(url)
        } else {
            Ok(())
        }
    }

    pub fn is_guardian_circuit_broken(&self) -> bool {
        self.guardian
            .as_ref()
            .map(|g| g.is_circuit_broken())
            .unwrap_or(false)
    }

    pub fn fs_policy(&self) -> Option<Arc<super::fs_policy::FileSystemPolicy>> {
        self.profile.fs_policy.clone()
    }

    pub fn register_permission_hook(&self, hook: Arc<dyn PermissionRequestHook>) {
        self.hook_registry.register_permission_hook(hook);
    }

    pub fn guardian_circuit_message(&self) -> Option<&'static str> {
        self.guardian
            .as_ref()
            .and_then(|g| g.circuit_breaker_message())
    }

    /// Record a permission decision in the audit trail.
    pub fn audit(
        &self,
        tool_name: &str,
        permission_key: &str,
        target: &str,
        decision: AuditDecision,
        layer: AuditLayer,
        reason: &str,
        matched_rule: Option<&str>,
    ) {
        self.audit_trail.record(
            tool_name,
            permission_key,
            target,
            decision,
            layer,
            reason,
            matched_rule,
        );
    }
}
