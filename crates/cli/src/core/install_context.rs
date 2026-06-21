//! Install method detection for Pick.

use std::path::PathBuf;
use std::sync::OnceLock;

const PICK_NPM_PACKAGE: &str = "@vividcodeai/pick";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandalonePlatform {
    Unix,
    Windows,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallMethod {
    Npm,
    GitHub { platform: StandalonePlatform },
    Cargo,
    Other,
}

#[derive(Debug, Clone)]
pub struct InstallContext {
    pub method: InstallMethod,
}

static INSTALL_CONTEXT: OnceLock<InstallContext> = OnceLock::new();

impl InstallContext {
    pub fn current() -> &'static InstallContext {
        INSTALL_CONTEXT.get_or_init(Self::detect)
    }

    fn detect() -> Self {
        let method = Self::detect_method();
        Self { method }
    }

    fn detect_method() -> InstallMethod {
        // Check env var override (set by npm postinstall script or similar)
        if std::env::var("PICK_MANAGED_BY_NPM").is_ok() {
            return InstallMethod::Npm;
        }

        let exe_path = match std::env::current_exe() {
            Ok(p) => p,
            Err(_) => return InstallMethod::Other,
        };

        let exe_str = exe_path.to_string_lossy().to_lowercase();

        // Check if installed in npm global prefix
        if let Some(npm_prefix) = get_npm_prefix() {
            let npm_bin = PathBuf::from(&npm_prefix).join("bin");
            if exe_path.starts_with(&npm_bin) || exe_str.contains("node_modules") {
                return InstallMethod::Npm;
            }
        }

        // Check if in cargo bin directory
        if let Ok(home) = std::env::var("CARGO_HOME") {
            let cargo_bin = PathBuf::from(home).join("bin");
            if exe_path.starts_with(&cargo_bin) {
                return InstallMethod::Cargo;
            }
        }
        let home = dirs::home_dir().unwrap_or_default();
        let cargo_default = home.join(".cargo").join("bin");
        if exe_path.starts_with(&cargo_default) {
            return InstallMethod::Cargo;
        }

        // Check if standalone — under ~/.pick/packages/standalone/ (symlink-resolved)
        // or under ~/.pick/bin/ (direct copy, e.g. Windows install.ps1 copy)
        let pick_home = home.join(".pick");
        let pick_standalone = pick_home.join("packages").join("standalone");
        let pick_bin = pick_home.join("bin");
        if exe_path.starts_with(&pick_standalone) || exe_path.starts_with(&pick_bin) {
            return InstallMethod::GitHub {
                platform: if cfg!(windows) {
                    StandalonePlatform::Windows
                } else {
                    StandalonePlatform::Unix
                },
            };
        }

        InstallMethod::Other
    }

    pub fn is_source_build(&self) -> bool {
        matches!(self.method, InstallMethod::Other)
    }
}

impl std::fmt::Display for InstallMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallMethod::Npm => write!(f, "npm ({PICK_NPM_PACKAGE})"),
            InstallMethod::GitHub {
                platform: StandalonePlatform::Unix,
            } => {
                write!(f, "standalone (Unix)")
            }
            InstallMethod::GitHub {
                platform: StandalonePlatform::Windows,
            } => {
                write!(f, "standalone (Windows)")
            }
            InstallMethod::Cargo => write!(f, "cargo install"),
            InstallMethod::Other => write!(f, "source build / other"),
        }
    }
}

fn get_npm_prefix() -> Option<String> {
    let output = std::process::Command::new("npm")
        .args(["-g", "prefix"])
        .output()
        .ok()?;
    if output.status.success() {
        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !s.is_empty() { Some(s) } else { None }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_method_display() {
        assert!(InstallMethod::Npm.to_string().contains("npm"));
        assert!(InstallMethod::Cargo.to_string().contains("cargo"));
        assert!(InstallMethod::Other.to_string().contains("source"));
    }
}
