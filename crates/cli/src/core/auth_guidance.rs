//! Auth guidance - helper messages for authentication setup

use crate::config;

const UNKNOWN_PROVIDER: &str = "unknown";

/// Get login help message for configuring providers
pub fn get_provider_login_help() -> String {
    format!(
        "Use /connect to log into a provider via OAuth or API key. See:\n  {}\n  {}",
        config::get_docs_path()
            .join("providers.md")
            .to_string_lossy(),
        config::get_docs_path().join("models.md").to_string_lossy(),
    )
}

/// Format message when no models are available
pub fn format_no_models_available_message() -> String {
    format!("No models available. {}", get_provider_login_help())
}

/// Format message when no model is selected
pub fn format_no_model_selected_message() -> String {
    format!(
        "No model selected.\n\n{}\n\nThen use /model to select a model.",
        get_provider_login_help()
    )
}

/// Format message when no API key is found for a provider
pub fn format_no_api_key_found_message(provider: &str) -> String {
    let provider_display = if provider == UNKNOWN_PROVIDER {
        "the selected model"
    } else {
        provider
    };
    format!(
        "No API key found for {}.\n\n{}",
        provider_display,
        get_provider_login_help()
    )
}
