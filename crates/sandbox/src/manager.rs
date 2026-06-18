use std::sync::Arc;

use pick_agent::permission::fs_policy::FileSystemPolicy;
use pick_agent::permission::sandbox::{Sandbox, SandboxConfig};

use crate::platforms::create_platform_sandbox;

pub struct SandboxManager {
    platform_sandbox: Box<dyn Sandbox>,
    #[allow(dead_code)]
    config: SandboxConfig,
}

impl SandboxManager {
    pub fn new(
        _fs_policy: Option<Arc<FileSystemPolicy>>,
        config: &SandboxConfig,
    ) -> Self {
        let sandbox = create_platform_sandbox(config);
        Self {
            platform_sandbox: sandbox,
            config: config.clone(),
        }
    }

    pub fn sandbox_type(&self) -> pick_agent::permission::sandbox::SandboxType {
        self.platform_sandbox.sandbox_type()
    }

    pub fn is_sandbox_available(&self) -> bool {
        self.platform_sandbox.is_available()
    }
}
