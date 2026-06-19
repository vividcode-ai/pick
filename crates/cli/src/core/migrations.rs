//! One-time migrations that run on startup.

use std::path::Path;

/// Run all migrations. Called once on startup.
pub fn run_migrations(cwd: &Path) -> RunMigrationsResult {
    let migrated_auth_providers = migrate_auth_to_auth_json();
    migrate_sessions_from_agent_root();
    migrate_tools_to_bin();
    migrate_keybindings_config_file();
    let deprecation_warnings = migrate_extension_system(cwd);
    RunMigrationsResult {
        migrated_auth_providers,
        deprecation_warnings,
    }
}

pub struct RunMigrationsResult {
    pub migrated_auth_providers: Vec<String>,
    pub deprecation_warnings: Vec<String>,
}

/// Migrate legacy oauth.json and settings.json apiKeys to auth.json
fn migrate_auth_to_auth_json() -> Vec<String> {
    let agent_dir = crate::config::get_agent_dir();
    let auth_path = agent_dir.join("auth.json");
    let oauth_path = agent_dir.join("oauth.json");
    let settings_path = agent_dir.join("settings.json");

    // Skip if auth.json already exists
    if auth_path.exists() {
        return Vec::new();
    }

    let mut migrated: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    let mut providers: Vec<String> = Vec::new();

    // Migrate oauth.json
    if oauth_path.exists()
        && let Ok(content) = std::fs::read_to_string(&oauth_path)
        && let Ok(oauth) = serde_json::from_str::<serde_json::Value>(&content)
        && let Some(obj) = oauth.as_object()
    {
        for (provider, cred) in obj {
            let entry = serde_json::json!({"type": "oauth", "cred": cred});
            migrated.insert(provider.clone(), entry);
            providers.push(provider.clone());
        }
        let _ = std::fs::rename(&oauth_path, oauth_path.with_extension("json.migrated"));
    }

    // Migrate settings.json apiKeys
    if settings_path.exists()
        && let Ok(content) = std::fs::read_to_string(&settings_path)
        && let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&content)
        && let Some(api_keys) = settings.get("apiKeys").and_then(|v| v.as_object())
    {
        for (provider, key) in api_keys {
            if !migrated.contains_key(provider) && key.is_string() {
                let entry = serde_json::json!({"type": "api_key", "key": key});
                migrated.insert(provider.clone(), entry);
                providers.push(provider.clone());
            }
        }
        if let Some(obj) = settings.as_object_mut() {
            obj.remove("apiKeys");
            if let Ok(new_content) = serde_json::to_string_pretty(&settings) {
                let _ = std::fs::write(&settings_path, new_content);
            }
        }
    }

    if !migrated.is_empty()
        && let Ok(_content) = serde_json::to_string_pretty(&migrated)
    {
        if let Some(parent) = auth_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(content) = serde_json::to_string_pretty(&migrated) {
                if let Ok(_) = std::fs::write(&auth_path, &content) {
                    let _ = std::fs::set_permissions(
                        &auth_path,
                        std::fs::Permissions::from_mode(0o600),
                    );
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = std::fs::write(&auth_path, &_content);
        }
    }

    providers
}

/// Migrate sessions from ~/.pick/*.jsonl to proper session directories
fn migrate_sessions_from_agent_root() {
    let agent_dir = crate::config::get_agent_dir();

    let files: Vec<_> = match std::fs::read_dir(&agent_dir) {
        Ok(entries) => entries
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
            .map(|e| e.path())
            .collect(),
        Err(_) => return,
    };

    if files.is_empty() {
        return;
    }

    for file in &files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let first_line = match content.lines().next() {
            Some(l) if !l.trim().is_empty() => l,
            _ => continue,
        };

        let header: serde_json::Value = match serde_json::from_str(first_line) {
            Ok(h) => h,
            Err(_) => continue,
        };

        if header.get("type").and_then(|v| v.as_str()) != Some("session") {
            continue;
        }

        let cwd = match header.get("cwd").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => continue,
        };

        // Compute safe directory path (same encoding as session-manager)
        let safe_path = format!(
            "--{}--",
            cwd.trim_start_matches('/')
                .trim_start_matches('\\')
                .replace(['/', '\\', ':'], "-")
        );
        let correct_dir = agent_dir.join("sessions").join(&safe_path);

        if !correct_dir.exists() {
            let _ = std::fs::create_dir_all(&correct_dir);
        }

        let file_name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let new_path = correct_dir.join(file_name);

        if new_path.exists() {
            continue;
        }

        let _ = std::fs::rename(file, &new_path);
    }
}

/// Move fd/rg binaries from tools/ to bin/
fn migrate_tools_to_bin() {
    let agent_dir = crate::config::get_agent_dir();
    let tools_dir = agent_dir.join("tools");
    let bin_dir = crate::config::get_agent_dir().join("bin"); // get_bin_dir equivalent

    if !tools_dir.exists() {
        return;
    }

    let binaries = if cfg!(windows) {
        ["fd.exe", "rg.exe"]
    } else {
        ["fd", "rg"]
    };
    let mut moved_any = false;

    for binary in &binaries {
        let old_path = tools_dir.join(binary);
        let new_path = bin_dir.join(binary);

        if old_path.exists() {
            if !bin_dir.exists() {
                let _ = std::fs::create_dir_all(&bin_dir);
            }
            if !new_path.exists() {
                if std::fs::rename(&old_path, &new_path).is_ok() {
                    moved_any = true;
                }
            } else {
                let _ = std::fs::remove_file(&old_path);
            }
        }
    }

    if moved_any {
        eprintln!("Migrated managed binaries tools/ → bin/");
    }
}

/// Migrate keybindings config file
fn migrate_keybindings_config_file() {
    let config_path = crate::config::get_agent_dir().join("keybindings.json");
    if !config_path.exists() {
        return;
    }

    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return,
    };

    if !parsed.is_object() {
        return;
    }

    let raw: std::collections::HashMap<String, serde_json::Value> =
        serde_json::from_value(parsed).unwrap_or_default();
    let result = crate::core::keybindings::migrate_keybindings_config(raw);

    if result.migrated
        && let Ok(new_content) = serde_json::to_string_pretty(&result.config)
    {
        let _ = std::fs::write(&config_path, format!("{}\n", new_content));
    }
}

/// Migrate commands/ to prompts/
fn migrate_commands_to_prompts(base_dir: &Path, label: &str) -> bool {
    let commands_dir = base_dir.join("commands");
    let prompts_dir = base_dir.join("prompts");

    if commands_dir.exists() && !prompts_dir.exists() {
        match std::fs::rename(&commands_dir, &prompts_dir) {
            Ok(_) => {
                eprintln!("Migrated {} commands/ → prompts/", label);
                true
            }
            Err(e) => {
                eprintln!(
                    "Warning: Could not migrate {} commands/ to prompts/: {}",
                    label, e
                );
                false
            }
        }
    } else {
        false
    }
}

/// Check for deprecated hooks/ and tools/ directories
fn check_deprecated_extension_dirs(base_dir: &Path, label: &str) -> Vec<String> {
    let mut warnings = Vec::new();

    let hooks_dir = base_dir.join("hooks");
    if hooks_dir.exists() {
        warnings.push(format!(
            "{} hooks/ directory found. Hooks have been renamed to extensions.",
            label
        ));
    }

    let tools_dir = base_dir.join("tools");
    if tools_dir.exists()
        && let Ok(entries) = std::fs::read_dir(&tools_dir)
    {
        let custom_tools: Vec<_> = entries
            .flatten()
            .filter(|e| {
                let lower = e.file_name().to_string_lossy().to_lowercase();
                lower != "fd"
                    && lower != "rg"
                    && lower != "fd.exe"
                    && lower != "rg.exe"
                    && !e.file_name().to_string_lossy().starts_with('.')
            })
            .collect();
        if !custom_tools.is_empty() {
            warnings.push(format!(
                    "{} tools/ directory contains custom tools. Custom tools have been merged into extensions.",
                    label
                ));
        }
    }

    warnings
}

/// Run extension system migrations and collect warnings
fn migrate_extension_system(cwd: &Path) -> Vec<String> {
    let agent_dir = crate::config::get_agent_dir();
    let project_dir = cwd.join(crate::config::CONFIG_DIR_NAME);

    migrate_commands_to_prompts(&agent_dir, "Global");
    migrate_commands_to_prompts(&project_dir, "Project");

    let mut warnings = check_deprecated_extension_dirs(&agent_dir, "Global");
    warnings.extend(check_deprecated_extension_dirs(&project_dir, "Project"));
    warnings
}
