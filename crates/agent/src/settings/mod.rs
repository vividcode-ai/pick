//! Settings types and persistence for Pick
//!
//! Supports two-tier merge: global (~/.pick/settings.json) + project (.pick/settings.json)

pub mod manager;
pub mod types;

pub use manager::SettingsManager;
pub use types::*;

use std::path::PathBuf;

/// Get path to global settings.json (~/.pick/settings.json)
pub fn get_global_settings_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_default();
    home.join(".pick").join("settings.json")
}

/// Get path to project-level settings.json (.pick/settings.json in the given cwd)
pub fn get_project_settings_path(cwd: &std::path::Path) -> PathBuf {
    cwd.join(".pick").join("settings.json")
}
