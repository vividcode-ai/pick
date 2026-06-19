use pick_tui::components::select::{SelectItem, SelectList};

use super::context::TuiContext;
use super::init;

/// Handle /model slash command
pub(crate) fn handle_model_command(ctx: &mut TuiContext, args: &[String]) {
    let search = args.join(" ").trim().to_string();

    // If exact match by model id, set directly (skip selector)
    if !search.is_empty() {
        let models = pick_ai::models::get_models(&ctx.provider);
        if let Some(exact) = models.iter().find(|m| m.id.eq_ignore_ascii_case(&search)) {
            ctx.model_id = exact.id.clone();
            let prov_str = exact.provider.as_str().to_string();
            ctx.provider = prov_str;
            init::save_default_model(&ctx.provider, &ctx.model_id);
            ctx.tui.model_id = ctx.model_id.clone();
            ctx.tui.provider = ctx.provider.clone();
            ctx.tui.chat.add_system_message(&format!(
                "Model set: \x1b[1m{}\x1b[0m (\x1b[2m{}\x1b[0m)",
                exact.id,
                exact.api.as_str()
            ));
        }
    }

    ctx.pending_command = Some("model".to_string());
    ctx.tui.chat.add_system_message("Select a model:");
    let models = pick_ai::models::get_models(&ctx.provider);
    let items: Vec<SelectItem> = if models.is_empty() {
        vec![
            SelectItem::new(ctx.model_id.clone(), ctx.model_id.clone()).with_description("Current"),
        ]
    } else {
        let filtered: Vec<_> = if search.is_empty() {
            models.iter().collect()
        } else {
            models
                .iter()
                .filter(|m| {
                    m.id.to_lowercase().contains(&search) || m.name.to_lowercase().contains(&search)
                })
                .collect()
        };
        filtered
            .iter()
            .map(|m| SelectItem::new(m.id.clone(), m.id.clone()).with_description(m.name.clone()))
            .collect()
    };
    if items.is_empty() {
        ctx.tui
            .chat
            .add_system_message("No models match your search.");
        ctx.pending_command = None;
    } else {
        let select = SelectList::new("Models", items);
        ctx.tui.start_selection(select);
    }
}

/// Handle /scoped-models slash command
pub(crate) fn handle_scoped_models_command(ctx: &mut TuiContext) {
    ctx.pending_command = Some("scoped-models".to_string());
    let models = pick_ai::models::get_models(&ctx.provider);
    if models.is_empty() {
        ctx.tui
            .chat
            .add_system_message("No models available for scoping.");
    } else {
        let items: Vec<SelectItem> = models
            .iter()
            .map(|m| {
                let enabled = ctx.scoped_models.contains(&m.id);
                let label = if enabled {
                    format!("[x] {}", m.id)
                } else {
                    format!("[ ] {}", m.id)
                };
                SelectItem::new(label, m.id.clone()).with_description(if enabled {
                    "enabled"
                } else {
                    "disabled"
                })
            })
            .collect();
        let status = if ctx.scoped_models.is_empty() {
            "All models available (none scoped)"
        } else {
            &format!("{} model(s) scoped", ctx.scoped_models.len())
        };
        let select = SelectList::new(format!("Scoped Models ({})", status), items);
        ctx.tui.start_selection(select);
    }
}

/// Handle /settings slash command: show settings menu
pub(crate) fn handle_settings_command(ctx: &mut TuiContext) {
    ctx.pending_command = Some("settings".to_string());
    let items = vec![
        SelectItem::new("Auto-compact", "auto-compact")
            .with_description("Toggle automatic context compaction"),
        SelectItem::new("Show images", "show-images")
            .with_description("Render images inline in terminal"),
        SelectItem::new("Image width", "image-width-cells")
            .with_description("Preferred inline image width in cells"),
        SelectItem::new("Auto-resize images", "auto-resize-images")
            .with_description("Resize large images to 2000x2000 max"),
        SelectItem::new("Block images", "block-images")
            .with_description("Prevent images from being sent to LLM"),
        SelectItem::new("Skill commands", "skill-commands")
            .with_description("Register skills as /skill:name commands"),
        SelectItem::new("Show hardware cursor", "show-hardware-cursor")
            .with_description("Show terminal cursor for IME support"),
        SelectItem::new("Editor padding", "editor-padding-x")
            .with_description("Horizontal padding for input editor (0-3)"),
        SelectItem::new("Autocomplete max items", "autocomplete-max-visible")
            .with_description("Max visible items in autocomplete"),
        SelectItem::new("Clear on shrink", "clear-on-shrink")
            .with_description("Clear empty rows when content shrinks"),
        SelectItem::new("Terminal progress", "terminal-progress")
            .with_description("Show progress in terminal tab bar"),
        SelectItem::new("Steering mode", "steering-mode")
            .with_description("How steering messages are delivered"),
        SelectItem::new("Follow-up mode", "follow-up-mode")
            .with_description("How follow-up messages are delivered"),
        SelectItem::new("Transport", "transport")
            .with_description("Preferred transport for providers"),
        SelectItem::new("HTTP idle timeout", "http-idle-timeout")
            .with_description("Max idle gap for HTTP requests"),
        SelectItem::new("Hide thinking", "hide-thinking")
            .with_description("Hide thinking blocks in responses"),
        SelectItem::new("Collapse changelog", "collapse-changelog")
            .with_description("Show condensed changelog after updates"),
        SelectItem::new("Quiet startup", "quiet-startup")
            .with_description("Disable verbose printing at startup"),
        SelectItem::new("Install telemetry", "install-telemetry")
            .with_description("Send anonymous version ping after updates"),
        SelectItem::new("Double-escape action", "double-escape-action")
            .with_description("Action on Esc twice with empty editor"),
        SelectItem::new("Tree filter mode", "tree-filter-mode")
            .with_description("Default filter when opening /tree"),
        SelectItem::new("Warnings", "warnings").with_description("Configure individual warnings"),
        SelectItem::new("Thinking level", "thinking").with_description("Change thinking level"),
        SelectItem::new("Theme", "theme").with_description("Change color theme"),
        SelectItem::new("Models", "models").with_description("Configure enabled models"),
    ];
    let select = SelectList::new("Settings", items);
    ctx.tui.start_selection(select);
}

/// Handle /login slash command
pub(crate) fn handle_login_command(ctx: &mut TuiContext) {
    ctx.pending_command = Some("login".to_string());
    let items = vec![
        SelectItem::new("Use an API key", "apikey").with_description("Sign in using an API key"),
        SelectItem::new("Use a subscription", "subscription")
            .with_description("Sign in with a subscription"),
    ];
    let select = SelectList::new("Select authentication method", items);
    ctx.tui.start_selection(select);
}

/// Handle /logout slash command
pub(crate) fn handle_logout_command(ctx: &mut TuiContext) {
    let providers = ctx.auth.list_providers();
    if providers.is_empty() {
        ctx.tui
            .chat
            .add_system_message("No stored credentials to remove.");
    } else {
        ctx.pending_command = Some("logout".to_string());
        let items: Vec<SelectItem> = providers
            .iter()
            .map(|p| {
                SelectItem::new(p.clone(), p.clone()).with_description("Remove stored credential")
            })
            .collect();
        let select = SelectList::new("Select provider to log out from", items);
        ctx.tui.start_selection(select);
    }
}
