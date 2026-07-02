//! Settings manager - re-exports from pick-agent with CLI-specific conveniences

use std::ops::{Deref, DerefMut};
use std::path::Path;

use crate::core::agent_session::RetryConfig;

/// CLI-specific SettingsManager wrapping the base pick-agent implementation.
/// Provides a `load(cwd)` convenience constructor using CLI path resolution.
#[derive(Debug)]
pub struct SettingsManager(pub pick_agent::settings::SettingsManager);

impl SettingsManager {
    /// Load settings with CLI path resolution (global ~/.pick/settings.json +
    /// project .pick/settings.json relative to `cwd`).
    pub fn load(cwd: &Path) -> Self {
        Self(pick_agent::settings::SettingsManager::load_from_paths(
            crate::config::get_settings_path(),
            cwd.join(crate::config::CONFIG_DIR_NAME)
                .join("settings.json"),
        ))
    }

    pub fn reload(&mut self, cwd: &Path) {
        let global_path = crate::config::get_settings_path();
        let project_path = cwd
            .join(crate::config::CONFIG_DIR_NAME)
            .join("settings.json");
        self.0.reload(&global_path, &project_path);
    }

    pub fn get_retry_settings(&self) -> RetryConfig {
        let s = self.0.get().retry.as_ref();
        RetryConfig {
            enabled: s.and_then(|r| r.enabled).unwrap_or(true),
            max_retries: s.and_then(|r| r.max_retries).unwrap_or(3),
            base_delay_ms: s.and_then(|r| r.base_delay_ms).unwrap_or(2000),
        }
    }
}

impl Deref for SettingsManager {
    type Target = pick_agent::settings::SettingsManager;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SettingsManager {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// Re-export all settings types from pick-agent so existing imports of
// `crate::core::settings::{Settings, CompactionSettings, ...}` continue to work.
pub use pick_agent::settings::*;
