use std::sync::atomic::Ordering;

use super::context::TuiContext;
use crate::core::settings::{
    CompactionSettings, ImageSettings, Settings, SettingsManager, TerminalSettings,
    WarningsSettings,
};

macro_rules! toggle_bool_setting {
    ($sm:expr, $ctx:expr, $field:ident, $label:expr) => {{
        let current = $sm.get().$field.unwrap_or(false);
        let mut update = Settings::default();
        update.$field = Some(!current);
        match $sm.set_global(update) {
            Ok(()) => {
                $ctx.tui.chat.add_system_message(&format!(
                    "{} \x1b[1m{}\x1b[0m.",
                    $label,
                    if !current { "enabled" } else { "disabled" }
                ));
            }
            Err(e) => $ctx
                .tui
                .show_error(&format!("Failed to save setting: {}", e)),
        }
    }};
}

pub(crate) fn toggle_enable_skill_commands(sm: &mut SettingsManager, ctx: &mut TuiContext) {
    toggle_bool_setting!(sm, ctx, enable_skill_commands, "Skill commands");
}

pub(crate) fn toggle_show_hardware_cursor(sm: &mut SettingsManager, ctx: &mut TuiContext) {
    let current = sm.get().show_hardware_cursor.unwrap_or(false);
    let new = !current;
    let mut update = Settings::default();
    update.show_hardware_cursor = Some(new);
    match sm.set_global(update) {
        Ok(()) => {
            ctx.tui.show_hardware_cursor = new;
            ctx.tui.chat.add_system_message(&format!(
                "Show hardware cursor \x1b[1m{}\x1b[0m.",
                if new { "enabled" } else { "disabled" }
            ));
        }
        Err(e) => ctx
            .tui
            .show_error(&format!("Failed to save setting: {}", e)),
    }
}

pub(crate) fn toggle_hide_thinking_block(sm: &mut SettingsManager, ctx: &mut TuiContext) {
    use std::sync::atomic::Ordering;
    let current = sm.get().hide_thinking_block.unwrap_or(false);
    let new = !current;
    let mut update = Settings::default();
    update.hide_thinking_block = Some(new);
    match sm.set_global(update) {
        Ok(()) => {
            ctx.hide_thinking.store(new, Ordering::Relaxed);
            ctx.tui.chat.add_system_message(&format!(
                "Show thinking \x1b[1m{}\x1b[0m.",
                if new { "disabled" } else { "enabled" }
            ));
        }
        Err(e) => ctx
            .tui
            .show_error(&format!("Failed to save setting: {}", e)),
    }
}

pub(crate) fn toggle_collapse_changelog(sm: &mut SettingsManager, ctx: &mut TuiContext) {
    toggle_bool_setting!(sm, ctx, collapse_changelog, "Collapse changelog");
}

pub(crate) fn toggle_quiet_startup(sm: &mut SettingsManager, ctx: &mut TuiContext) {
    toggle_bool_setting!(sm, ctx, quiet_startup, "Quiet startup");
}

pub(crate) fn toggle_enable_install_telemetry(sm: &mut SettingsManager, ctx: &mut TuiContext) {
    toggle_bool_setting!(sm, ctx, enable_install_telemetry, "Install telemetry");
}

pub(crate) async fn toggle_compact(sm: &mut SettingsManager, ctx: &mut TuiContext) {
    let current = sm
        .get()
        .compaction
        .as_ref()
        .and_then(|c| c.enabled)
        .unwrap_or(true);
    let new_enabled = !current;
    let mut update = Settings::default();
    update.compaction = Some(CompactionSettings {
        enabled: Some(new_enabled),
        reserve_tokens: None,
        keep_recent_tokens: None,
    });
    match sm.set_global(update) {
        Ok(()) => {
            ctx.tui.auto_compact = new_enabled;
            ctx.tui.chat.add_system_message(&format!(
                "Auto-compact \x1b[1m{}\x1b[0m.",
                if new_enabled { "enabled" } else { "disabled" }
            ));
        }
        Err(e) => ctx
            .tui
            .show_error(&format!("Failed to save setting: {}", e)),
    }
}

pub(crate) async fn toggle_show_images(sm: &mut SettingsManager, ctx: &mut TuiContext) {
    let current = sm
        .get()
        .terminal
        .as_ref()
        .and_then(|t| t.show_images)
        .unwrap_or(true);
    let new = !current;
    let mut update = Settings::default();
    update.terminal = Some(TerminalSettings {
        show_images: Some(new),
        image_width_cells: None,
        clear_on_shrink: None,
        show_terminal_progress: None,
    });
    match sm.set_global(update) {
        Ok(()) => {
            ctx.show_images.store(new, Ordering::Relaxed);
            ctx.tui.chat.add_system_message(&format!(
                "Show images \x1b[1m{}\x1b[0m.",
                if new { "enabled" } else { "disabled" }
            ));
        }
        Err(e) => ctx
            .tui
            .show_error(&format!("Failed to save setting: {}", e)),
    }
}

pub(crate) async fn toggle_auto_resize_images(sm: &mut SettingsManager, ctx: &mut TuiContext) {
    let current = sm
        .get()
        .images
        .as_ref()
        .and_then(|i| i.auto_resize)
        .unwrap_or(true);
    let mut update = Settings::default();
    update.images = Some(ImageSettings {
        auto_resize: Some(!current),
        block_images: None,
    });
    match sm.set_global(update) {
        Ok(()) => ctx.tui.chat.add_system_message(&format!(
            "Auto-resize images \x1b[1m{}\x1b[0m.",
            if !current { "enabled" } else { "disabled" }
        )),
        Err(e) => ctx
            .tui
            .show_error(&format!("Failed to save setting: {}", e)),
    }
}

pub(crate) async fn toggle_block_images(sm: &mut SettingsManager, ctx: &mut TuiContext) {
    let current = sm
        .get()
        .images
        .as_ref()
        .and_then(|i| i.block_images)
        .unwrap_or(false);
    let new = !current;
    let mut update = Settings::default();
    update.images = Some(ImageSettings {
        auto_resize: None,
        block_images: Some(new),
    });
    match sm.set_global(update) {
        Ok(()) => {
            ctx.block_images.store(new, Ordering::Relaxed);
            ctx.tui.chat.add_system_message(&format!(
                "Block images \x1b[1m{}\x1b[0m.",
                if new { "enabled" } else { "disabled" }
            ));
        }
        Err(e) => ctx
            .tui
            .show_error(&format!("Failed to save setting: {}", e)),
    }
}

pub(crate) async fn toggle_clear_on_shrink(sm: &mut SettingsManager, ctx: &mut TuiContext) {
    let current = sm
        .get()
        .terminal
        .as_ref()
        .and_then(|t| t.clear_on_shrink)
        .unwrap_or(false);
    let mut update = Settings::default();
    update.terminal = Some(TerminalSettings {
        show_images: None,
        image_width_cells: None,
        clear_on_shrink: Some(!current),
        show_terminal_progress: None,
    });
    match sm.set_global(update) {
        Ok(()) => ctx.tui.chat.add_system_message(&format!(
            "Clear on shrink \x1b[1m{}\x1b[0m.",
            if !current { "enabled" } else { "disabled" }
        )),
        Err(e) => ctx
            .tui
            .show_error(&format!("Failed to save setting: {}", e)),
    }
}

pub(crate) async fn toggle_terminal_progress(sm: &mut SettingsManager, ctx: &mut TuiContext) {
    let current = sm
        .get()
        .terminal
        .as_ref()
        .and_then(|t| t.show_terminal_progress)
        .unwrap_or(false);
    let mut update = Settings::default();
    update.terminal = Some(TerminalSettings {
        show_images: None,
        image_width_cells: None,
        clear_on_shrink: None,
        show_terminal_progress: Some(!current),
    });
    match sm.set_global(update) {
        Ok(()) => ctx.tui.chat.add_system_message(&format!(
            "Terminal progress \x1b[1m{}\x1b[0m.",
            if !current { "enabled" } else { "disabled" }
        )),
        Err(e) => ctx
            .tui
            .show_error(&format!("Failed to save setting: {}", e)),
    }
}

// ---- Apply setting values ----

pub(crate) async fn apply_thinking_level(
    sm: &mut SettingsManager,
    ctx: &mut TuiContext,
    val: &str,
) {
    let level = val.trim_start_matches("thinking-");
    let mut update = Settings::default();
    update.default_thinking_level = Some(level.to_string());
    match sm.set_global(update) {
        Ok(()) => {
            ctx.thinking_level = match level {
                "low" => pick_agent::core::state::ThinkingLevel::Low,
                "medium" => pick_agent::core::state::ThinkingLevel::Medium,
                "high" => pick_agent::core::state::ThinkingLevel::High,
                _ => pick_agent::core::state::ThinkingLevel::Off,
            };
            ctx.tui.thinking_level = level.to_string();
            ctx.tui
                .chat
                .add_system_message(&format!("Thinking level set to \x1b[1m{}\x1b[0m.", level));
        }
        Err(e) => ctx
            .tui
            .show_error(&format!("Failed to save setting: {}", e)),
    }
}

pub(crate) async fn apply_theme(sm: &mut SettingsManager, ctx: &mut TuiContext, val: &str) {
    let theme = val.trim_start_matches("theme-");
    let mut update = Settings::default();
    update.theme = Some(theme.to_string());
    match sm.set_global(update) {
        Ok(()) => ctx.tui.chat.add_system_message(&format!(
            "Theme set to \x1b[1m{}\x1b[0m (requires restart).",
            theme
        )),
        Err(e) => ctx
            .tui
            .show_error(&format!("Failed to save setting: {}", e)),
    }
}

pub(crate) async fn apply_image_width(sm: &mut SettingsManager, ctx: &mut TuiContext, val: &str) {
    let width_str = val.trim_start_matches("image-width-");
    if let Ok(width) = width_str.parse::<u32>() {
        let mut update = Settings::default();
        update.terminal = Some(TerminalSettings {
            show_images: None,
            image_width_cells: Some(width),
            clear_on_shrink: None,
            show_terminal_progress: None,
        });
        match sm.set_global(update) {
            Ok(()) => ctx.tui.chat.add_system_message(&format!(
                "Image width set to \x1b[1m{}\x1b[0m cells.",
                width
            )),
            Err(e) => ctx
                .tui
                .show_error(&format!("Failed to save setting: {}", e)),
        }
    }
}

pub(crate) async fn apply_editor_padding(
    sm: &mut SettingsManager,
    ctx: &mut TuiContext,
    val: &str,
) {
    let pad_str = val.trim_start_matches("editor-padding-");
    if let Ok(pad) = pad_str.parse::<u32>() {
        let mut update = Settings::default();
        update.editor_padding_x = Some(pad);
        match sm.set_global(update) {
            Ok(()) => ctx
                .tui
                .chat
                .add_system_message(&format!("Editor padding set to \x1b[1m{}\x1b[0m.", pad)),
            Err(e) => ctx
                .tui
                .show_error(&format!("Failed to save setting: {}", e)),
        }
    }
}

pub(crate) async fn apply_ac_max(sm: &mut SettingsManager, ctx: &mut TuiContext, val: &str) {
    let n_str = val.trim_start_matches("ac-max-");
    if let Ok(n) = n_str.parse::<u32>() {
        let mut update = Settings::default();
        update.autocomplete_max_visible = Some(n);
        match sm.set_global(update) {
            Ok(()) => ctx.tui.chat.add_system_message(&format!(
                "Autocomplete max items set to \x1b[1m{}\x1b[0m.",
                n
            )),
            Err(e) => ctx
                .tui
                .show_error(&format!("Failed to save setting: {}", e)),
        }
    }
}

pub(crate) async fn apply_steering_mode(sm: &mut SettingsManager, ctx: &mut TuiContext, val: &str) {
    let mode = val.trim_start_matches("steering-");
    let mut update = Settings::default();
    update.steering_mode = Some(mode.to_string());
    match sm.set_global(update) {
        Ok(()) => ctx
            .tui
            .chat
            .add_system_message(&format!("Steering mode set to \x1b[1m{}\x1b[0m.", mode)),
        Err(e) => ctx
            .tui
            .show_error(&format!("Failed to save setting: {}", e)),
    }
}

pub(crate) async fn apply_follow_up_mode(
    sm: &mut SettingsManager,
    ctx: &mut TuiContext,
    val: &str,
) {
    let mode = val.trim_start_matches("followup-");
    let mut update = Settings::default();
    update.follow_up_mode = Some(mode.to_string());
    match sm.set_global(update) {
        Ok(()) => ctx
            .tui
            .chat
            .add_system_message(&format!("Follow-up mode set to \x1b[1m{}\x1b[0m.", mode)),
        Err(e) => ctx
            .tui
            .show_error(&format!("Failed to save setting: {}", e)),
    }
}

pub(crate) async fn apply_transport(sm: &mut SettingsManager, ctx: &mut TuiContext, val: &str) {
    let transport = val.trim_start_matches("transport-");
    let mut update = Settings::default();
    update.transport = Some(transport.to_string());
    match sm.set_global(update) {
        Ok(()) => ctx
            .tui
            .chat
            .add_system_message(&format!("Transport set to \x1b[1m{}\x1b[0m.", transport)),
        Err(e) => ctx
            .tui
            .show_error(&format!("Failed to save setting: {}", e)),
    }
}

pub(crate) async fn apply_http_timeout(sm: &mut SettingsManager, ctx: &mut TuiContext, val: &str) {
    let ms_str = val.trim_start_matches("http-timeout-");
    if let Ok(ms) = ms_str.parse::<u64>() {
        let mut update = Settings::default();
        update.http_idle_timeout_ms = Some(ms);
        match sm.set_global(update) {
            Ok(()) => {
                if ms == 0 {
                    ctx.tui
                        .chat
                        .add_system_message("HTTP idle timeout \x1b[1mdisabled\x1b[0m.");
                } else {
                    ctx.tui.chat.add_system_message(&format!(
                        "HTTP idle timeout set to \x1b[1m{}ms\x1b[0m.",
                        ms
                    ));
                }
            }
            Err(e) => ctx
                .tui
                .show_error(&format!("Failed to save setting: {}", e)),
        }
    }
}

pub(crate) async fn apply_de_action(sm: &mut SettingsManager, ctx: &mut TuiContext, val: &str) {
    let action = val.trim_start_matches("de-");
    let mut update = Settings::default();
    update.double_escape_action = Some(action.to_string());
    match sm.set_global(update) {
        Ok(()) => ctx.tui.chat.add_system_message(&format!(
            "Double-escape action set to \x1b[1m{}\x1b[0m.",
            action
        )),
        Err(e) => ctx
            .tui
            .show_error(&format!("Failed to save setting: {}", e)),
    }
}

pub(crate) async fn apply_tree_filter(sm: &mut SettingsManager, ctx: &mut TuiContext, val: &str) {
    let mode = val.trim_start_matches("tf-");
    let mut update = Settings::default();
    update.tree_filter_mode = Some(mode.to_string());
    match sm.set_global(update) {
        Ok(()) => ctx
            .tui
            .chat
            .add_system_message(&format!("Tree filter mode set to \x1b[1m{}\x1b[0m.", mode)),
        Err(e) => ctx
            .tui
            .show_error(&format!("Failed to save setting: {}", e)),
    }
}

pub(crate) async fn apply_warning(sm: &mut SettingsManager, ctx: &mut TuiContext, val: &str) {
    let rest = val.trim_start_matches("warnings-");
    if let Some(warning_id) = rest
        .strip_suffix("-true")
        .or_else(|| rest.strip_suffix("-false"))
    {
        let bool_val = rest.ends_with("-true");
        match warning_id {
            "anthropic-extra-usage" => {
                let mut update = Settings::default();
                update.warnings = Some(WarningsSettings {
                    anthropic_extra_usage: Some(bool_val),
                });
                match sm.set_global(update) {
                    Ok(()) => ctx.tui.chat.add_system_message(&format!(
                        "Anthropic extra usage warning \x1b[1m{}\x1b[0m.",
                        if bool_val { "enabled" } else { "disabled" }
                    )),
                    Err(e) => ctx
                        .tui
                        .show_error(&format!("Failed to save setting: {}", e)),
                }
            }
            _ => ctx
                .tui
                .chat
                .add_system_message(&format!("Unknown warning: {}", warning_id)),
        }
    }
}
