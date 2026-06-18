//! Agent configuration and settings

use std::path::PathBuf;

/// Agent configuration
pub struct AgentConfig {
    pub app_name: String,
    pub config_dir_name: String,
    pub version: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            app_name: "Pick".to_string(),
            config_dir_name: ".pick".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

impl AgentConfig {
    /// Get the agent config directory (e.g., ~/.pick/agent/)
    pub fn get_agent_dir(&self) -> PathBuf {
        dirs::home_dir()
            .unwrap_or_default()
            .join(&self.config_dir_name)
            .join("agent")
    }

    /// Get path to settings.json
    pub fn get_settings_path(&self) -> PathBuf {
        self.get_agent_dir().join("settings.json")
    }

    /// Get path to auth.json
    pub fn get_auth_path(&self) -> PathBuf {
        self.get_agent_dir().join("auth.json")
    }

    /// Get path to models.json
    pub fn get_models_path(&self) -> PathBuf {
        self.get_agent_dir().join("models.json")
    }

    /// Get path to sessions directory
    pub fn get_sessions_dir(&self) -> PathBuf {
        self.get_agent_dir().join("sessions")
    }
}
