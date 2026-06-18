//! Settings manager - two-tier (global + project) settings


pub mod types;

use std::path::PathBuf;

use fs2::FileExt;
use pick_agent::permission::approval::PermissionConfig;

pub use types::*;

fn merge_primitive(base: &mut Settings, overrides: &Settings) {
    macro_rules! merge_opt {
        ($field:ident) => {
            if overrides.$field.is_some() {
                base.$field = overrides.$field.clone();
            }
        };
    }
    merge_opt!(last_changelog_version);
    merge_opt!(default_provider);
    merge_opt!(default_model);
    merge_opt!(default_thinking_level);
    merge_opt!(transport);
    merge_opt!(theme);
    merge_opt!(shell_path);
    merge_opt!(shell_command_prefix);
    merge_opt!(npm_command);
    merge_opt!(quiet_startup);
    merge_opt!(hide_thinking_block);
    merge_opt!(collapse_changelog);
    merge_opt!(enable_skill_commands);
    merge_opt!(steering_mode);
    merge_opt!(follow_up_mode);
    merge_opt!(double_escape_action);
    merge_opt!(tree_filter_mode);
    merge_opt!(show_hardware_cursor);
    merge_opt!(editor_padding_x);
    merge_opt!(autocomplete_max_visible);
    merge_opt!(http_idle_timeout_ms);
    merge_opt!(enabled_models);
    merge_opt!(extensions);
    merge_opt!(skills);
    merge_opt!(session_dir);
    merge_opt!(enable_install_telemetry);
    merge_opt!(check_for_update_on_startup);
    merge_opt!(dismissed_update_version);
    merge_opt!(mcp_servers);
    merge_opt!(permission);
    merge_opt!(packages);
    merge_opt!(prompts);
    merge_opt!(themes);
}

fn merge_object_fields(base: &mut Settings, overrides: &Settings) {
    if let Some(ref o) = overrides.compaction {
        base.compaction = Some(match &base.compaction {
            Some(b) => CompactionSettings {
                enabled: o.enabled.or(b.enabled),
                reserve_tokens: o.reserve_tokens.or(b.reserve_tokens),
                keep_recent_tokens: o.keep_recent_tokens.or(b.keep_recent_tokens),
            },
            None => o.clone(),
        });
    }
    if let Some(ref o) = overrides.retry {
        base.retry = Some(match &base.retry {
            Some(b) => RetrySettings {
                enabled: o.enabled.or(b.enabled),
                max_retries: o.max_retries.or(b.max_retries),
                base_delay_ms: o.base_delay_ms.or(b.base_delay_ms),
                provider: match (&o.provider, &b.provider) {
                    (Some(op), Some(bp)) => Some(ProviderRetrySettings {
                        timeout_ms: op.timeout_ms.or(bp.timeout_ms),
                        max_retries: op.max_retries.or(bp.max_retries),
                        max_retry_delay_ms: op.max_retry_delay_ms.or(bp.max_retry_delay_ms),
                    }),
                    (Some(op), None) => Some(op.clone()),
                    (None, Some(_)) => b.provider.clone(),
                    (None, None) => None,
                },
            },
            None => o.clone(),
        });
    }
    if let Some(ref o) = overrides.branch_summary {
        base.branch_summary = Some(match &base.branch_summary {
            Some(b) => BranchSummarySettings {
                reserve_tokens: o.reserve_tokens.or(b.reserve_tokens),
                skip_prompt: o.skip_prompt.or(b.skip_prompt),
            },
            None => o.clone(),
        });
    }
    if let Some(ref o) = overrides.markdown {
        base.markdown = Some(match &base.markdown {
            Some(b) => MarkdownSettings {
                code_block_indent: o.code_block_indent.clone().or(b.code_block_indent.clone()),
            },
            None => o.clone(),
        });
    }
    if let Some(ref o) = overrides.terminal {
        base.terminal = Some(match &base.terminal {
            Some(b) => TerminalSettings {
                show_images: o.show_images.or(b.show_images),
                image_width_cells: o.image_width_cells.or(b.image_width_cells),
                clear_on_shrink: o.clear_on_shrink.or(b.clear_on_shrink),
                show_terminal_progress: o.show_terminal_progress.or(b.show_terminal_progress),
            },
            None => o.clone(),
        });
    }
    if let Some(ref o) = overrides.images {
        base.images = Some(match &base.images {
            Some(b) => ImageSettings {
                auto_resize: o.auto_resize.or(b.auto_resize),
                block_images: o.block_images.or(b.block_images),
            },
            None => o.clone(),
        });
    }
    if let Some(ref o) = overrides.thinking_budgets {
        base.thinking_budgets = Some(match &base.thinking_budgets {
            Some(b) => ThinkingBudgetsSettings {
                minimal: o.minimal.or(b.minimal),
                low: o.low.or(b.low),
                medium: o.medium.or(b.medium),
                high: o.high.or(b.high),
            },
            None => o.clone(),
        });
    }
    if let Some(ref o) = overrides.warnings {
        base.warnings = Some(match &base.warnings {
            Some(b) => WarningsSettings {
                anthropic_extra_usage: o.anthropic_extra_usage.or(b.anthropic_extra_usage),
            },
            None => o.clone(),
        });
    }
}

fn deep_merge(base: &Settings, overrides: &Settings) -> Settings {
    let mut result = base.clone();
    merge_primitive(&mut result, overrides);
    merge_object_fields(&mut result, overrides);

    for (key, value) in &overrides.extra {
        result.extra.insert(key.clone(), value.clone());
    }

    result
}

pub struct SettingsManager {
    global_path: PathBuf,
    project_path: PathBuf,
    global_settings: Settings,
    project_settings: Settings,
    merged: Settings,
}

impl SettingsManager {
    pub fn load(cwd: &std::path::Path) -> Self {
        let global_path = crate::config::get_settings_path();
        let project_path = cwd.join(crate::config::CONFIG_DIR_NAME).join("settings.json");

        let global_settings = load_settings_from_file(&global_path);
        let project_settings = load_settings_from_file(&project_path);
        let merged = deep_merge(&global_settings, &project_settings);

        Self {
            global_path,
            project_path,
            global_settings,
            project_settings,
            merged,
        }
    }

    pub fn reload(&mut self, cwd: &std::path::Path) {
        let global_settings = load_settings_from_file(&self.global_path);
        let project_settings_path = cwd.join(crate::config::CONFIG_DIR_NAME).join("settings.json");
        let project_settings = if project_settings_path != self.project_path {
            load_settings_from_file(&project_settings_path)
        } else {
            load_settings_from_file(&self.project_path)
        };
        self.project_path = project_settings_path;
        self.global_settings = global_settings;
        self.project_settings = project_settings;
        self.merged = deep_merge(&self.global_settings, &self.project_settings);
    }

    pub fn get(&self) -> &Settings {
        &self.merged
    }

    pub fn get_global(&self) -> &Settings {
        &self.global_settings
    }

    pub fn get_project(&self) -> &Settings {
        &self.project_settings
    }

    pub fn set_global(&mut self, settings: Settings) -> Result<(), String> {
        self.global_settings = deep_merge(&self.global_settings, &settings);
        self.merged = deep_merge(&self.global_settings, &self.project_settings);
        self.save_global()
    }

    pub fn set_project(&mut self, settings: Settings) -> Result<(), String> {
        self.project_settings = deep_merge(&self.project_settings, &settings);
        self.merged = deep_merge(&self.global_settings, &self.project_settings);
        self.save_project()
    }

    pub fn default_provider(&self) -> Option<&str> {
        self.merged.default_provider.as_deref()
    }

    pub fn default_model(&self) -> Option<&str> {
        self.merged.default_model.as_deref()
    }

    pub fn default_thinking_level(&self) -> Option<&str> {
        self.merged.default_thinking_level.as_deref()
    }

    pub fn transport(&self) -> Option<&str> {
        self.merged.transport.as_deref()
    }

    pub fn theme(&self) -> Option<&str> {
        self.merged.theme.as_deref()
    }

    pub fn shell_path(&self) -> Option<&str> {
        self.merged.shell_path.as_deref()
    }

    pub fn session_dir(&self) -> Option<&std::path::Path> {
        self.merged.session_dir.as_ref().map(std::path::Path::new)
    }

    pub fn get_quiet_startup(&self) -> bool {
        self.merged.quiet_startup.unwrap_or(false)
    }

    pub fn get_hide_thinking_block(&self) -> bool {
        self.merged.hide_thinking_block.unwrap_or(false)
    }

    pub fn get_collapse_changelog(&self) -> bool {
        self.merged.collapse_changelog.unwrap_or(false)
    }

    pub fn get_enable_skill_commands(&self) -> bool {
        self.merged.enable_skill_commands.unwrap_or(true)
    }

    pub fn get_steering_mode(&self) -> &str {
        self.merged.steering_mode.as_deref().unwrap_or("one-at-a-time")
    }

    pub fn get_follow_up_mode(&self) -> &str {
        self.merged.follow_up_mode.as_deref().unwrap_or("one-at-a-time")
    }

    pub fn get_double_escape_action(&self) -> &str {
        self.merged.double_escape_action.as_deref().unwrap_or("tree")
    }

    pub fn get_tree_filter_mode(&self) -> &str {
        self.merged.tree_filter_mode.as_deref().unwrap_or("default")
    }

    pub fn get_show_hardware_cursor(&self) -> bool {
        self.merged.show_hardware_cursor.unwrap_or(false)
    }

    pub fn get_editor_padding_x(&self) -> u32 {
        self.merged.editor_padding_x.unwrap_or(0)
    }

    pub fn get_autocomplete_max_visible(&self) -> u32 {
        self.merged.autocomplete_max_visible.unwrap_or(5)
    }

    pub fn get_clear_on_shrink(&self) -> bool {
        self.merged.terminal.as_ref().and_then(|t| t.clear_on_shrink).unwrap_or(false)
    }

    pub fn get_show_terminal_progress(&self) -> bool {
        self.merged.terminal.as_ref().and_then(|t| t.show_terminal_progress).unwrap_or(false)
    }

    pub fn get_http_idle_timeout_ms(&self) -> u64 {
        self.merged.http_idle_timeout_ms.unwrap_or(300_000)
    }

    pub fn get_show_images(&self) -> bool {
        self.merged.terminal.as_ref().and_then(|t| t.show_images).unwrap_or(true)
    }

    pub fn get_image_width_cells(&self) -> u32 {
        self.merged.terminal.as_ref().and_then(|t| t.image_width_cells).unwrap_or(60)
    }

    pub fn get_image_auto_resize(&self) -> bool {
        self.merged.images.as_ref().and_then(|i| i.auto_resize).unwrap_or(true)
    }

    pub fn get_block_images(&self) -> bool {
        self.merged.images.as_ref().and_then(|i| i.block_images).unwrap_or(false)
    }

    pub fn get_retry_enabled(&self) -> bool {
        self.merged.retry.as_ref()
            .and_then(|r| r.enabled).unwrap_or(true)
    }

    pub fn get_retry_settings(&self) -> super::agent_session::RetryConfig {
        let s = self.merged.retry.as_ref();
        super::agent_session::RetryConfig {
            enabled: s.and_then(|r| r.enabled).unwrap_or(true),
            max_retries: s.and_then(|r| r.max_retries).unwrap_or(3),
            base_delay_ms: s.and_then(|r| r.base_delay_ms).unwrap_or(2000),
        }
    }

    pub fn get_check_for_update_on_startup(&self) -> bool {
        self.merged.check_for_update_on_startup.unwrap_or(true)
    }

    pub fn get_permission(&self) -> PermissionConfig {
        self.merged.permission.clone().unwrap_or_default()
    }

    pub fn get_warnings(&self) -> WarningsSettings {
        self.merged.warnings.clone().unwrap_or(WarningsSettings {
            anthropic_extra_usage: Some(true),
        })
    }

    fn save_global(&self) -> Result<(), String> {
        save_settings_to_file(&self.global_path, &self.global_settings)
    }

    fn save_project(&self) -> Result<(), String> {
        save_settings_to_file(&self.project_path, &self.project_settings)
    }
}

fn load_settings_from_file(path: &std::path::Path) -> Settings {
    if !path.exists() {
        return Settings::default();
    }
    match std::fs::read_to_string(path) {
        Ok(content) => {
            serde_json::from_str(&content).unwrap_or_else(|e| {
                tracing::warn!("Failed to parse settings file {:?}: {}", path, e);
                Settings::default()
            })
        }
        Err(e) => {
            tracing::warn!("Failed to read settings file {:?}: {}", path, e);
            Settings::default()
        }
    }
}

fn save_settings_to_file(path: &std::path::Path, settings: &Settings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create settings dir: {}", e))?;
    }
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|e| format!("Failed to open settings file: {}", e))?;
    file.lock_exclusive().map_err(|e| format!("Failed to lock settings file: {}", e))?;
    use std::io::Write;
    file.write_all(content.as_bytes()).map_err(|e| format!("Failed to write settings: {}", e))?;
    file.unlock().map_err(|e| format!("Failed to unlock settings file: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deep_merge() {
        let base = Settings {
            default_provider: Some("anthropic".to_string()),
            default_model: Some("claude-sonnet-4-20250514".to_string()),
            ..Default::default()
        };

        let overrides = Settings {
            default_model: Some("claude-opus-4-20250514".to_string()),
            ..Default::default()
        };

        let merged = deep_merge(&base, &overrides);
        assert_eq!(merged.default_provider.as_deref(), Some("anthropic"));
        assert_eq!(merged.default_model.as_deref(), Some("claude-opus-4-20250514"));
    }

    #[test]
    fn test_deep_merge_nested() {
        let base = Settings {
            compaction: Some(CompactionSettings {
                enabled: Some(true),
                reserve_tokens: Some(16384),
                keep_recent_tokens: None,
            }),
            ..Default::default()
        };

        let overrides = Settings {
            compaction: Some(CompactionSettings {
                enabled: Some(false),
                reserve_tokens: None,
                keep_recent_tokens: Some(20000),
            }),
            ..Default::default()
        };

        let merged = deep_merge(&base, &overrides);
        let c = merged.compaction.unwrap();
        assert_eq!(c.enabled, Some(false));
        assert_eq!(c.reserve_tokens, Some(16384));
        assert_eq!(c.keep_recent_tokens, Some(20000));
    }
}
