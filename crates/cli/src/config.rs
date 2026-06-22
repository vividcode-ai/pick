//! CLI configuration constants

pub const APP_NAME: &str = "Pick";
pub const APP_TITLE: &str = "Pick";
pub const CONFIG_DIR_NAME: &str = ".pick";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

use std::path::PathBuf;

/// Get the top-level Pick config directory (~/.pick/)
pub fn get_pick_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_default();
    home.join(CONFIG_DIR_NAME)
}

/// Get the agent config directory
pub fn get_agent_dir() -> PathBuf {
    get_pick_dir().join("agent")
}

/// Get the sessions directory
pub fn get_sessions_dir() -> PathBuf {
    get_agent_dir().join("sessions")
}

/// Get path to global settings.json (~/.pick/settings.json)
pub fn get_settings_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_default();
    home.join(CONFIG_DIR_NAME).join("settings.json")
}

/// Get path to auth.json
pub fn get_auth_path() -> PathBuf {
    get_agent_dir().join("auth.json")
}

/// Get path to docs directory
pub fn get_docs_path() -> PathBuf {
    get_agent_dir().join("docs")
}

/// Get path to built-in themes directory
pub fn get_themes_dir() -> PathBuf {
    get_agent_dir().join("themes")
}

/// Get path to custom themes directory (project-level)
pub fn get_custom_themes_dir(project_dir: &std::path::Path) -> PathBuf {
    project_dir.join(CONFIG_DIR_NAME).join("themes")
}

/// Get path to README.md
pub fn get_readme_path() -> PathBuf {
    get_agent_dir().join("README.md")
}

/// Get path to examples directory
pub fn get_examples_path() -> PathBuf {
    get_agent_dir().join("examples")
}
