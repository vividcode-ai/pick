use pick_agent::permission::sandbox::{SandboxConfig, SandboxRequest, Sandbox, SandboxType};

pub struct NullSandbox;

impl NullSandbox {
    pub fn new(_config: &SandboxConfig) -> Self {
        Self
    }
}

impl Sandbox for NullSandbox {
    fn sandbox_type(&self) -> SandboxType {
        SandboxType::None
    }

    fn name(&self) -> &str {
        "none"
    }

    fn is_available(&self) -> bool {
        false
    }

    fn transform(
        &self,
        _req: &SandboxRequest,
    ) -> Result<(String, Vec<String>), String> {
        Err("No sandbox available on this platform".into())
    }
}
