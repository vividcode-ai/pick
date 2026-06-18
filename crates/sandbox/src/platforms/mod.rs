use pick_agent::permission::sandbox::{Sandbox, SandboxConfig};

#[cfg(target_os = "linux")]
pub mod linux_bwrap;
#[cfg(target_os = "macos")]
pub mod macos_seatbelt;
#[cfg(target_os = "windows")]
pub mod windows_restricted_token;

mod null_sandbox;

pub fn create_platform_sandbox(config: &SandboxConfig) -> Box<dyn Sandbox> {
    #[cfg(target_os = "linux")]
    {
        let s = linux_bwrap::LinuxBwrapSandbox::new(config);
        if s.is_available() {
            return Box::new(s);
        }
    }
    #[cfg(target_os = "macos")]
    {
        let s = macos_seatbelt::MacosSeatbeltSandbox::new(config);
        if s.is_available() {
            return Box::new(s);
        }
    }
    #[cfg(target_os = "windows")]
    {
        let s = windows_restricted_token::WindowsRestrictedTokenSandbox::new(config);
        if s.is_available() {
            return Box::new(s);
        }
    }
    Box::new(null_sandbox::NullSandbox::new(config))
}
