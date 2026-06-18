//! Update action definitions for Pick.


use super::install_context::{InstallContext, InstallMethod, StandalonePlatform};

const PICK_NPM_PACKAGE: &str = "@vividcodeai/pick";
const PICK_REPO: &str = "https://github.com/vividcodeai/pick";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateAction {
    NpmGlobalLatest,
    GitHubStandaloneUnix,
    GitHubStandaloneWindows,
    CargoInstall,
    Manual,
}

impl UpdateAction {
    pub fn from_install_context(ctx: &InstallContext) -> Self {
        match ctx.method {
            InstallMethod::Npm => Self::NpmGlobalLatest,
            InstallMethod::GitHub { platform: StandalonePlatform::Unix } => {
                Self::GitHubStandaloneUnix
            }
            InstallMethod::GitHub { platform: StandalonePlatform::Windows } => {
                Self::GitHubStandaloneWindows
            }
            InstallMethod::Cargo => Self::CargoInstall,
            InstallMethod::Other => Self::Manual,
        }
    }

    pub fn command_args(&self) -> (&'static str, Vec<&'static str>) {
        match self {
            Self::NpmGlobalLatest => ("npm", vec!["install", "-g", PICK_NPM_PACKAGE]),
            Self::GitHubStandaloneUnix => {
                ("sh", vec!["-c", "curl -fsSL https://vividcodeai.github.io/pick/install.sh | sh"])
            }
            Self::GitHubStandaloneWindows => {
                ("powershell", vec![
                    "-ExecutionPolicy", "Bypass",
                    "-c", "irm https://vividcodeai.github.io/pick/install.ps1 | iex",
                ])
            }
            Self::CargoInstall => {
                ("cargo", vec!["install", "pick", "--git", PICK_REPO])
            }
            Self::Manual => ("", vec![]),
        }
    }

    pub fn command_str(&self) -> String {
        match self {
            Self::NpmGlobalLatest => format!("npm install -g {PICK_NPM_PACKAGE}"),
            Self::GitHubStandaloneUnix => {
                "curl -fsSL https://vividcodeai.github.io/pick/install.sh | sh".to_string()
            }
            Self::GitHubStandaloneWindows => {
                "irm https://vividcodeai.github.io/pick/install.ps1 | iex".to_string()
            }
            Self::CargoInstall => format!("cargo install pick --git {PICK_REPO}"),
            Self::Manual => {
                "Manually download from https://github.com/vividcodeai/pick/releases/latest".to_string()
            }
        }
    }
}

pub fn get_update_action() -> Option<UpdateAction> {
    let ctx = InstallContext::current();
    let action = UpdateAction::from_install_context(ctx);
    if matches!(action, UpdateAction::Manual) {
        None
    } else {
        Some(action)
    }
}
