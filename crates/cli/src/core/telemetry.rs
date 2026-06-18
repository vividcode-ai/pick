//! Telemetry - install telemetry configuration


use crate::core::settings::SettingsManager;

fn is_truthy_env_flag(value: &str) -> bool {
    matches!(value, "1" | "true" | "yes" | "TRUE" | "YES")
}

/// Check if install telemetry is enabled
pub fn is_install_telemetry_enabled(
    settings_manager: &SettingsManager,
) -> bool {
    let env_var = std::env::var("PI_TELEMETRY").unwrap_or_default();
    if !env_var.is_empty() {
        return is_truthy_env_flag(&env_var);
    }
    settings_manager.get().enable_install_telemetry.unwrap_or(false)
}
