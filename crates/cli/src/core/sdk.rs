//! SDK - main entry point for creating agent sessions

use std::path::PathBuf;

use crate::core::defaults::DEFAULT_THINKING_LEVEL;
use crate::core::model_registry::ModelRegistry;
use crate::core::settings::SettingsManager;
use pick_agent::session::SessionManager;

/// Options for creating an agent session
#[derive(Default)]
pub struct CreateAgentSessionOptions {
    /// Working directory for project-local discovery
    pub cwd: Option<String>,
    /// Global config directory
    pub agent_dir: Option<String>,
    /// Model registry
    pub model_registry: Option<ModelRegistry>,
    /// Session manager
    pub session_manager: Option<SessionManager>,
    /// Settings manager
    pub settings_manager: Option<SettingsManager>,
}


/// Result from creating an agent session
pub struct CreateAgentSessionResult {
    /// The created session ID
    pub session_id: String,
    /// Diagnostics collected during setup
    pub diagnostics: Vec<String>,
    /// Model fallback warning message
    pub model_fallback_message: Option<String>,
    /// The session manager instance
    pub session_manager: SessionManager,
    /// The extensions result info
    pub extensions_result: ExtensionsResult,
}

/// Extension loading result (simplified)
#[derive(Clone)]
pub struct ExtensionsResult {
    pub paths: Vec<String>,
    pub info: String,
}

/// Tool definition (simplified)
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Create an agent session with the specified options
pub async fn create_agent_session(
    options: CreateAgentSessionOptions,
) -> Result<CreateAgentSessionResult, String> {
    let cwd = options.cwd.unwrap_or_else(|| {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default()
    });

    let agent_dir = options
        .agent_dir
        .unwrap_or_else(|| crate::config::get_agent_dir().to_string_lossy().to_string());

    let model_registry = options.model_registry.unwrap_or_else(|| {
        let auth_storage = crate::core::auth_storage::AuthStorage::create(Some(
            PathBuf::from(&agent_dir).join("auth.json"),
        ));
        ModelRegistry::create(
            auth_storage,
            Some(PathBuf::from(&agent_dir).join("models.json")),
        )
    });

    let settings_manager = options
        .settings_manager
        .unwrap_or_else(|| SettingsManager::load(std::path::Path::new(&cwd)));

    let session_manager = match options.session_manager {
        Some(sm) => sm,
        None => {
            let session_dir = get_default_session_dir(&cwd, &agent_dir);
            let cwd_path = std::path::PathBuf::from(&cwd);
            SessionManager::create(cwd_path, Some(session_dir))
                .await
                .unwrap_or_else(|e| panic!("Failed to create session: {}", e))
        }
    };

    let diagnostics: Vec<String> = Vec::new();
    let model_fallback_message: Option<String> = None;

    // Determine model from session or settings
    let model = model_registry.find(
        settings_manager.default_provider().unwrap_or("anthropic"),
        settings_manager
            .default_model()
            .unwrap_or("claude-sonnet-4-6"),
    );

    let _model = model.or_else(|| model_registry.get_available().first().cloned());

    let _thinking_level = settings_manager
        .default_thinking_level()
        .unwrap_or(DEFAULT_THINKING_LEVEL)
        .to_string();

    let session_id = session_manager
        .header()
        .map(|h| h.id.clone())
        .unwrap_or_default();

    Ok(CreateAgentSessionResult {
        session_id,
        diagnostics,
        model_fallback_message,
        session_manager,
        extensions_result: ExtensionsResult {
            paths: Vec::new(),
            info: String::new(),
        },
    })
}

/// Compute the default session directory for a cwd
pub fn get_default_session_dir(cwd: &str, agent_dir: &str) -> PathBuf {
    crate::core::session_manager::get_default_session_dir(cwd, agent_dir)
}
