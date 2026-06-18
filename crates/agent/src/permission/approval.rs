use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalPolicy {
    /// Ask for approval of everything except known-safe commands
    UnlessTrusted,
    /// Auto-approve everything; escalate to user on sandbox failure
    OnFailure,
    /// Model decides when to ask
    OnRequest,
    /// Fine-grained control via GranularApprovalConfig
    Granular,
    /// Never ask; fail if blocked
    Never,
}

impl Default for ApprovalPolicy {
    fn default() -> Self {
        Self::OnRequest
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GranularApprovalConfig {
    #[serde(default)]
    pub sandbox_approval: bool,
    #[serde(default)]
    pub rules: bool,
    #[serde(default)]
    pub skill_approval: bool,
    #[serde(default)]
    pub request_permissions: bool,
    #[serde(default)]
    pub mcp_elicitations: bool,
}

impl Default for GranularApprovalConfig {
    fn default() -> Self {
        Self {
            sandbox_approval: true,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: true,
        }
    }
}

/// Complete permission configuration that can be merged from settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PermissionConfig {
    #[serde(default)]
    pub approval_policy: ApprovalPolicy,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub granular: Option<GranularApprovalConfig>,

    #[serde(default = "default_permission_profile")]
    pub permission_profile: String,
}

fn default_permission_profile() -> String {
    ":workspace".to_string()
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self {
            approval_policy: ApprovalPolicy::default(),
            granular: None,
            permission_profile: default_permission_profile(),
        }
    }
}

impl PermissionConfig {
    pub fn should_prompt(&self, is_sandbox_issue: bool) -> bool {
        match self.approval_policy {
            ApprovalPolicy::Never => false,
            ApprovalPolicy::OnRequest => true,
            ApprovalPolicy::OnFailure => is_sandbox_issue,
            ApprovalPolicy::UnlessTrusted => {
                // Static context: always return false — the runtime decision
                // requires the command context and is handled by PermissionManager.
                false
            }
            ApprovalPolicy::Granular => {
                let g = self.granular.as_ref().cloned().unwrap_or_default();
                if is_sandbox_issue {
                    g.sandbox_approval
                } else {
                    g.rules
                }
            }
        }
    }

    pub fn should_deny_on_prompt(&self) -> bool {
        matches!(self.approval_policy, ApprovalPolicy::Never)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approval_policy_default() {
        let config = PermissionConfig::default();
        assert_eq!(config.approval_policy, ApprovalPolicy::OnRequest);
        assert_eq!(config.permission_profile, ":workspace");
    }

    #[test]
    fn test_never_policy_denies() {
        let config = PermissionConfig {
            approval_policy: ApprovalPolicy::Never,
            ..Default::default()
        };
        assert!(config.should_deny_on_prompt());
        assert!(!config.should_prompt(false));
    }

    #[test]
    fn test_on_failure_only_prompts_on_sandbox() {
        let config = PermissionConfig {
            approval_policy: ApprovalPolicy::OnFailure,
            ..Default::default()
        };
        assert!(!config.should_prompt(false));
        assert!(config.should_prompt(true));
    }

    #[test]
    fn test_granular_config() {
        let config = PermissionConfig {
            approval_policy: ApprovalPolicy::Granular,
            granular: Some(GranularApprovalConfig {
                sandbox_approval: false,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(!config.should_prompt(true));
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = PermissionConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: PermissionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }
}
