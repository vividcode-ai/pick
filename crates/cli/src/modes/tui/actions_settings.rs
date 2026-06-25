use crate::core::settings::SettingsManager;
use pick_tui::components::select::{SelectItem, SelectList};

use super::context::TuiContext;
use super::settings_values;

/// Handle settings selection result
pub(crate) async fn handle_settings_selection(ctx: &mut TuiContext, val: &str) {
    let cwd_set = std::env::current_dir().unwrap_or_default();
    let mut sm = SettingsManager::load(&cwd_set);

    match val {
        "auto-compact" => settings_values::toggle_compact(&mut sm, ctx).await,
        "sandbox" => settings_values::toggle_sandbox_enabled(&mut sm, ctx).await,
        "show-images" => settings_values::toggle_show_images(&mut sm, ctx).await,
        "auto-resize-images" => settings_values::toggle_auto_resize_images(&mut sm, ctx).await,
        "block-images" => settings_values::toggle_block_images(&mut sm, ctx).await,
        "skill-commands" => settings_values::toggle_enable_skill_commands(&mut sm, ctx),
        "show-hardware-cursor" => settings_values::toggle_show_hardware_cursor(&mut sm, ctx),
        "clear-on-shrink" => settings_values::toggle_clear_on_shrink(&mut sm, ctx).await,
        "terminal-progress" => settings_values::toggle_terminal_progress(&mut sm, ctx).await,
        "hide-thinking" => settings_values::toggle_hide_thinking_block(&mut sm, ctx),
        "collapse-changelog" => settings_values::toggle_collapse_changelog(&mut sm, ctx),
        "quiet-startup" => settings_values::toggle_quiet_startup(&mut sm, ctx),
        "install-telemetry" => settings_values::toggle_enable_install_telemetry(&mut sm, ctx),
        "image-width-cells" => show_image_width_selector(ctx, &sm),
        "editor-padding-x" => show_editor_padding_selector(ctx, &sm),
        "autocomplete-max-visible" => show_ac_max_visible_selector(ctx, &sm),
        "steering-mode" => show_steering_mode_selector(ctx, &sm),
        "follow-up-mode" => show_follow_up_mode_selector(ctx, &sm),
        "transport" => show_transport_selector(ctx, &sm),
        "http-idle-timeout" => show_http_timeout_selector(ctx, &sm),
        "double-escape-action" => show_double_escape_selector(ctx, &sm),
        "tree-filter-mode" => show_tree_filter_selector(ctx, &sm),
        "warnings" => show_warnings_selector(ctx, &sm),
        "thinking" => {
            let lvl = ctx.thinking_level;
            show_thinking_selector(ctx, &sm, &lvl);
        }
        "theme" => show_theme_selector(ctx, &sm),
        "models" => {
            let p = ctx.provider.clone();
            let m = ctx.model_id.clone();
            show_models_selector(ctx, &sm, &p, &m);
        }
        other if other.starts_with("thinking-") => {
            settings_values::apply_thinking_level(&mut sm, ctx, other).await
        }
        other if other.starts_with("theme-") => {
            settings_values::apply_theme(&mut sm, ctx, other).await
        }
        other if other.starts_with("image-width-") => {
            settings_values::apply_image_width(&mut sm, ctx, other).await
        }
        other if other.starts_with("editor-padding-") => {
            settings_values::apply_editor_padding(&mut sm, ctx, other).await
        }
        other if other.starts_with("ac-max-") => {
            settings_values::apply_ac_max(&mut sm, ctx, other).await
        }
        other if other.starts_with("steering-") => {
            settings_values::apply_steering_mode(&mut sm, ctx, other).await
        }
        other if other.starts_with("followup-") => {
            settings_values::apply_follow_up_mode(&mut sm, ctx, other).await
        }
        other if other.starts_with("transport-") => {
            settings_values::apply_transport(&mut sm, ctx, other).await
        }
        other if other.starts_with("http-timeout-") => {
            settings_values::apply_http_timeout(&mut sm, ctx, other).await
        }
        other if other.starts_with("de-") => {
            settings_values::apply_de_action(&mut sm, ctx, other).await
        }
        other if other.starts_with("tf-") => {
            settings_values::apply_tree_filter(&mut sm, ctx, other).await
        }
        other if other.starts_with("warnings-") => {
            settings_values::apply_warning(&mut sm, ctx, other).await
        }
        _ => ctx
            .tui
            .chat
            .add_system_message(&format!("Unknown setting: {}", val)),
    }
}

// ---- Selector helpers ----

fn show_image_width_selector(ctx: &mut TuiContext, sm: &SettingsManager) {
    ctx.pending_command = Some("settings".to_string());
    let current = sm
        .get()
        .terminal
        .as_ref()
        .and_then(|t| t.image_width_cells)
        .unwrap_or(80);
    let items = vec![
        SelectItem::new("60", "image-width-60").with_description(if current == 60 {
            "current"
        } else {
            ""
        }),
        SelectItem::new("80", "image-width-80").with_description(if current == 80 {
            "current"
        } else {
            ""
        }),
        SelectItem::new("120", "image-width-120").with_description(if current == 120 {
            "current"
        } else {
            ""
        }),
    ];
    let select = SelectList::new("Image Width (cells)", items);
    ctx.tui.start_selection(select);
    ctx.tui.finalize_turn();
}

fn show_editor_padding_selector(ctx: &mut TuiContext, sm: &SettingsManager) {
    ctx.pending_command = Some("settings".to_string());
    let current = sm.get_editor_padding_x();
    let items = vec![
        SelectItem::new("0", "editor-padding-0").with_description(if current == 0 {
            "current"
        } else {
            ""
        }),
        SelectItem::new("1", "editor-padding-1").with_description(if current == 1 {
            "current"
        } else {
            ""
        }),
        SelectItem::new("2", "editor-padding-2").with_description(if current == 2 {
            "current"
        } else {
            ""
        }),
        SelectItem::new("3", "editor-padding-3").with_description(if current == 3 {
            "current"
        } else {
            ""
        }),
    ];
    let select = SelectList::new("Editor Padding", items);
    ctx.tui.start_selection(select);
    ctx.tui.finalize_turn();
}

fn show_ac_max_visible_selector(ctx: &mut TuiContext, sm: &SettingsManager) {
    ctx.pending_command = Some("settings".to_string());
    let current = sm.get_autocomplete_max_visible();
    let items = vec![
        SelectItem::new("3", "ac-max-3").with_description(if current == 3 {
            "current"
        } else {
            ""
        }),
        SelectItem::new("5", "ac-max-5").with_description(if current == 5 {
            "current"
        } else {
            ""
        }),
        SelectItem::new("7", "ac-max-7").with_description(if current == 7 {
            "current"
        } else {
            ""
        }),
        SelectItem::new("10", "ac-max-10").with_description(if current == 10 {
            "current"
        } else {
            ""
        }),
        SelectItem::new("15", "ac-max-15").with_description(if current == 15 {
            "current"
        } else {
            ""
        }),
        SelectItem::new("20", "ac-max-20").with_description(if current == 20 {
            "current"
        } else {
            ""
        }),
    ];
    let select = SelectList::new("Autocomplete Max Items", items);
    ctx.tui.start_selection(select);
    ctx.tui.finalize_turn();
}

fn show_steering_mode_selector(ctx: &mut TuiContext, sm: &SettingsManager) {
    ctx.pending_command = Some("settings".to_string());
    let current = sm.get_steering_mode().to_string();
    let items = vec![
        SelectItem::new("one-at-a-time", "steering-one-at-a-time").with_description(
            if current == "one-at-a-time" {
                "current"
            } else {
                ""
            },
        ),
        SelectItem::new("all", "steering-all").with_description(if current == "all" {
            "current"
        } else {
            ""
        }),
    ];
    let select = SelectList::new("Steering Mode", items);
    ctx.tui.start_selection(select);
    ctx.tui.finalize_turn();
}

fn show_follow_up_mode_selector(ctx: &mut TuiContext, sm: &SettingsManager) {
    ctx.pending_command = Some("settings".to_string());
    let current = sm.get_follow_up_mode().to_string();
    let items = vec![
        SelectItem::new("one-at-a-time", "followup-one-at-a-time").with_description(
            if current == "one-at-a-time" {
                "current"
            } else {
                ""
            },
        ),
        SelectItem::new("all", "followup-all").with_description(if current == "all" {
            "current"
        } else {
            ""
        }),
    ];
    let select = SelectList::new("Follow-up Mode", items);
    ctx.tui.start_selection(select);
    ctx.tui.finalize_turn();
}

fn show_transport_selector(ctx: &mut TuiContext, sm: &SettingsManager) {
    ctx.pending_command = Some("settings".to_string());
    let current = sm.transport().unwrap_or("auto");
    let items = vec![
        SelectItem::new("sse", "transport-sse").with_description(if current == "sse" {
            "current"
        } else {
            ""
        }),
        SelectItem::new("websocket", "transport-websocket").with_description(
            if current == "websocket" {
                "current"
            } else {
                ""
            },
        ),
        SelectItem::new("websocket-cached", "transport-websocket-cached").with_description(
            if current == "websocket-cached" {
                "current"
            } else {
                ""
            },
        ),
        SelectItem::new("auto", "transport-auto").with_description(if current == "auto" {
            "current"
        } else {
            ""
        }),
    ];
    let select = SelectList::new("Transport", items);
    ctx.tui.start_selection(select);
    ctx.tui.finalize_turn();
}

fn show_http_timeout_selector(ctx: &mut TuiContext, sm: &SettingsManager) {
    ctx.pending_command = Some("settings".to_string());
    let current = sm.get_http_idle_timeout_ms();
    let pairs = [
        ("30s", 30000),
        ("1min", 60000),
        ("5min", 300000),
        ("10min", 600000),
        ("30min", 1800000),
        ("Disabled", 0),
    ];
    let items: Vec<SelectItem> = pairs
        .iter()
        .map(|(label, ms)| {
            SelectItem::new(*label, format!("http-timeout-{}", ms))
                .with_description(if *ms == current { "current" } else { "" })
        })
        .collect();
    let select = SelectList::new("HTTP Idle Timeout", items);
    ctx.tui.start_selection(select);
    ctx.tui.finalize_turn();
}

fn show_double_escape_selector(ctx: &mut TuiContext, sm: &SettingsManager) {
    ctx.pending_command = Some("settings".to_string());
    let current = sm.get_double_escape_action().to_string();
    let items = vec![
        SelectItem::new("tree", "de-tree").with_description(if current == "tree" {
            "current"
        } else {
            ""
        }),
        SelectItem::new("fork", "de-fork").with_description(if current == "fork" {
            "current"
        } else {
            ""
        }),
        SelectItem::new("none", "de-none").with_description(if current == "none" {
            "current"
        } else {
            ""
        }),
    ];
    let select = SelectList::new("Double-Escape Action", items);
    ctx.tui.start_selection(select);
    ctx.tui.finalize_turn();
}

fn show_tree_filter_selector(ctx: &mut TuiContext, sm: &SettingsManager) {
    ctx.pending_command = Some("settings".to_string());
    let current = sm.get_tree_filter_mode().to_string();
    let items = vec![
        SelectItem::new("default", "tf-default").with_description(if current == "default" {
            "current"
        } else {
            ""
        }),
        SelectItem::new("no-tools", "tf-no-tools").with_description(if current == "no-tools" {
            "current"
        } else {
            ""
        }),
        SelectItem::new("user-only", "tf-user-only").with_description(if current == "user-only" {
            "current"
        } else {
            ""
        }),
        SelectItem::new("labeled-only", "tf-labeled-only").with_description(
            if current == "labeled-only" {
                "current"
            } else {
                ""
            },
        ),
        SelectItem::new("all", "tf-all").with_description(if current == "all" {
            "current"
        } else {
            ""
        }),
    ];
    let select = SelectList::new("Tree Filter Mode", items);
    ctx.tui.start_selection(select);
    ctx.tui.finalize_turn();
}

fn show_warnings_selector(ctx: &mut TuiContext, sm: &SettingsManager) {
    ctx.pending_command = Some("settings".to_string());
    let warnings = sm.get_warnings();
    let current_extra = warnings.anthropic_extra_usage.unwrap_or(true);
    let items = vec![
        SelectItem::new(
            if current_extra { "Disable" } else { "Enable" },
            if current_extra {
                "warnings-anthropic-extra-usage-false"
            } else {
                "warnings-anthropic-extra-usage-true"
            },
        )
        .with_description("Anthropic extra usage warning"),
    ];
    let select = SelectList::new("Warnings", items);
    ctx.tui.start_selection(select);
    ctx.tui.finalize_turn();
}

fn show_thinking_selector(
    ctx: &mut TuiContext,
    _sm: &SettingsManager,
    current_level: &pick_agent::core::state::ThinkingLevel,
) {
    ctx.pending_command = Some("settings".to_string());
    let levels = ["off", "low", "medium", "high"];
    let current_str = format!("{:?}", current_level).to_lowercase();
    let items: Vec<SelectItem> = levels
        .iter()
        .map(|l| {
            let desc = if *l == current_str { "current" } else { "" };
            SelectItem::new(l.to_string(), format!("thinking-{}", l)).with_description(desc)
        })
        .collect();
    let select = SelectList::new("Thinking Level", items);
    ctx.tui.start_selection(select);
    ctx.tui.finalize_turn();
}

fn show_theme_selector(ctx: &mut TuiContext, sm: &SettingsManager) {
    ctx.pending_command = Some("settings".to_string());
    let themes = ["dark", "light", "solarized-dark", "solarized-light"];
    let current = sm.get().theme.as_deref().unwrap_or("dark");
    let items: Vec<SelectItem> = themes
        .iter()
        .map(|t| {
            let desc = if *t == current { "current" } else { "" };
            SelectItem::new(t.to_string(), format!("theme-{}", t)).with_description(desc)
        })
        .collect();
    let select = SelectList::new("Theme", items);
    ctx.tui.start_selection(select);
    ctx.tui.finalize_turn();
}

fn show_models_selector(
    ctx: &mut TuiContext,
    _sm: &SettingsManager,
    provider: &str,
    model_id: &str,
) {
    ctx.pending_command = Some("settings-models".to_string());
    let models = pick_ai::models::get_models(provider);
    let items: Vec<SelectItem> = if models.is_empty() {
        vec![
            SelectItem::new(model_id.to_string(), model_id.to_string())
                .with_description("Current model"),
        ]
    } else {
        models
            .iter()
            .map(|m| {
                let is_current = m.id == model_id;
                SelectItem::new(m.id.clone(), format!("{}/{}", m.provider.as_str(), m.id))
                    .with_description(if is_current { "current" } else { &m.name })
            })
            .collect()
    };
    let select = SelectList::new("Select default model", items);
    ctx.tui.start_selection(select);
    ctx.tui.finalize_turn();
}
