pub use pick_agent::permission::sandbox::{
    Sandbox as PlatformSandbox,
    SandboxRequest,
    SandboxType,
};

#[derive(Debug)]
pub enum SandboxError {
    Denied { path: String, reason: String },
    Unavailable { reason: String },
    SpawnFailed { reason: String },
    Timeout,
}

impl std::fmt::Display for SandboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SandboxError::Denied { path, reason } => {
                write!(f, "Sandbox denied access to '{}': {}", path, reason)
            }
            SandboxError::Unavailable { reason } => {
                write!(f, "Sandbox unavailable: {}", reason)
            }
            SandboxError::SpawnFailed { reason } => {
                write!(f, "Sandbox spawn failed: {}", reason)
            }
            SandboxError::Timeout => write!(f, "Sandbox command timed out"),
        }
    }
}

impl From<SandboxError> for String {
    fn from(e: SandboxError) -> Self {
        e.to_string()
    }
}
