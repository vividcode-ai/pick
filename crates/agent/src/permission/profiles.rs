use std::path::Path;
use std::sync::Arc;

use super::approval::{ApprovalPolicy, PermissionConfig};
use super::fs_policy::FileSystemPolicy;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProfileKind {
    ReadOnly,
    Workspace,
    DangerFullAccess,
}

impl ProfileKind {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            ":read-only" | "read-only" | "readonly" => Some(Self::ReadOnly),
            ":workspace" | "workspace" => Some(Self::Workspace),
            ":danger-full-access" | "danger-full-access" | "full-access" | "full" => {
                Some(Self::DangerFullAccess)
            }
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ReadOnly => ":read-only",
            Self::Workspace => ":workspace",
            Self::DangerFullAccess => ":danger-full-access",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NetworkAccess {
    Blocked,
    Restricted,
    ProxyOnly,
    Full,
}

#[derive(Debug, Clone)]
pub struct PermissionProfile {
    pub kind: ProfileKind,
    pub fs_policy: Option<Arc<FileSystemPolicy>>,
    pub network_access: NetworkAccess,
    pub exec_policy_enabled: bool,
    pub guardian_enabled: bool,
    pub sandbox_enabled: bool,
    pub approval_policy: ApprovalPolicy,
    pub protected_metadata_paths: Vec<String>,
}

impl PermissionProfile {
    pub fn resolve(
        profile_str: &str,
        workspace_root: &Path,
        config: Option<&PermissionConfig>,
    ) -> Self {
        let kind = ProfileKind::from_str(profile_str).unwrap_or(ProfileKind::Workspace);
        let approval = config
            .map(|c| c.approval_policy)
            .unwrap_or(ApprovalPolicy::OnRequest);

        let protected = super::fs_policy::default_protected_paths();

        match kind {
            ProfileKind::ReadOnly => Self {
                kind,
                fs_policy: Some(Arc::new(FileSystemPolicy::new_readonly(workspace_root))),
                network_access: NetworkAccess::Restricted,
                exec_policy_enabled: true,
                guardian_enabled: true,
                sandbox_enabled: true,
                approval_policy: approval,
                protected_metadata_paths: protected,
            },
            ProfileKind::Workspace => Self {
                kind,
                fs_policy: Some(Arc::new(FileSystemPolicy::new_workspace_default(
                    workspace_root,
                ))),
                network_access: NetworkAccess::Restricted,
                exec_policy_enabled: true,
                guardian_enabled: true,
                sandbox_enabled: true,
                approval_policy: approval,
                protected_metadata_paths: protected,
            },
            ProfileKind::DangerFullAccess => Self {
                kind,
                fs_policy: Some(Arc::new(FileSystemPolicy::new_full_access())),
                network_access: NetworkAccess::Full,
                exec_policy_enabled: false,
                guardian_enabled: false,
                sandbox_enabled: false,
                approval_policy: approval,
                protected_metadata_paths: Vec::new(),
            },
        }
    }

    pub fn fs_policy(&self) -> Option<Arc<FileSystemPolicy>> {
        self.fs_policy.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_kind_from_str() {
        assert_eq!(ProfileKind::from_str(":read-only"), Some(ProfileKind::ReadOnly));
        assert_eq!(ProfileKind::from_str(":workspace"), Some(ProfileKind::Workspace));
        assert_eq!(
            ProfileKind::from_str(":danger-full-access"),
            Some(ProfileKind::DangerFullAccess)
        );
        assert_eq!(ProfileKind::from_str("invalid"), None);
    }

    #[test]
    fn test_resolve_workspace_profile() {
        let tmp = std::env::temp_dir();
        let profile = PermissionProfile::resolve(":workspace", &tmp, None);
        assert_eq!(profile.kind, ProfileKind::Workspace);
        assert!(profile.sandbox_enabled);
        assert!(profile.guardian_enabled);
        assert_eq!(profile.network_access, NetworkAccess::Restricted);
    }

    #[test]
    fn test_resolve_readonly_profile() {
        let tmp = std::env::temp_dir();
        let profile = PermissionProfile::resolve(":read-only", &tmp, None);
        assert_eq!(profile.kind, ProfileKind::ReadOnly);
        if let Some(ref fs) = profile.fs_policy {
            assert!(!fs.allow_absolute_paths);
        } else {
            panic!("Expected fs_policy");
        }
    }

    #[test]
    fn test_resolve_full_access() {
        let tmp = std::env::temp_dir();
        let profile = PermissionProfile::resolve(":danger-full-access", &tmp, None);
        assert_eq!(profile.kind, ProfileKind::DangerFullAccess);
        assert!(!profile.guardian_enabled);
        assert!(!profile.sandbox_enabled);
        assert_eq!(profile.network_access, NetworkAccess::Full);
    }
}
