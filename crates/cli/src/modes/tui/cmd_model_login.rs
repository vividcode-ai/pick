use pick_tui::components::select::{SelectItem, SelectList};

use super::context::TuiContext;
use super::init;
use crate::core::settings::SettingsManager;

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

    // Load current settings to display boolean states
    let cwd_set = std::env::current_dir().unwrap_or_default();
    let sm = SettingsManager::load(&cwd_set);

    let s = |enabled: bool| -> &'static str { if enabled { "enabled" } else { "disabled" } };

    let auto_compact = sm
        .get()
        .compaction
        .as_ref()
        .and_then(|c| c.enabled)
        .unwrap_or(true);
    let show_images = sm.get_show_images();
    let auto_resize = sm.get_image_auto_resize();
    let block_images = sm.get_block_images();
    let skill_cmds = sm.get_enable_skill_commands();
    let hw_cursor = sm.get_show_hardware_cursor();
    let clear_shrink = sm.get_clear_on_shrink();
    let term_prog = sm.get_show_terminal_progress();
    let show_thinking = !sm.get_hide_thinking_block(); // inverted: "Show thinking" = enabled when NOT hiding
    let show_tool_calls = !sm.get_hide_tool_call_block(); // inverted
    let collapse = sm.get_collapse_changelog();
    let quiet = sm.get_quiet_startup();
    let telemetry = sm.get().enable_install_telemetry.unwrap_or(false);
    let sandbox = sm.get_permission().sandbox_enabled;
    let mcp_tools = sm.get_enable_mcp_tools();
    let notif = sm.get_enable_system_notifications();
    let tep = sm
        .get()
        .tool_execution_permission
        .as_deref()
        .unwrap_or("prompt")
        .to_string();

    let items = vec![
        SelectItem::new(
            format!("Auto-compact  [{}]", s(auto_compact)),
            "auto-compact",
        ),
        SelectItem::new(format!("Sandbox  [{}]", s(sandbox)), "sandbox"),
        SelectItem::new(
            format!("Tool exec permission  [{}]", tep),
            "tool-execution-permission",
        ),
        SelectItem::new(format!("MCP tools  [{}]", s(mcp_tools)), "mcp-tools"),
        SelectItem::new(
            format!("System notifications  [{}]", s(notif)),
            "system-notifications",
        ),
        SelectItem::new(format!("Show images  [{}]", s(show_images)), "show-images"),
        SelectItem::new("Image width", "image-width-cells"),
        SelectItem::new(
            format!("Auto-resize images  [{}]", s(auto_resize)),
            "auto-resize-images",
        ),
        SelectItem::new(
            format!("Block images  [{}]", s(block_images)),
            "block-images",
        ),
        SelectItem::new(
            format!("Skill commands  [{}]", s(skill_cmds)),
            "skill-commands",
        ),
        SelectItem::new(
            format!("Show hardware cursor  [{}]", s(hw_cursor)),
            "show-hardware-cursor",
        ),
        SelectItem::new("Editor padding", "editor-padding-x"),
        SelectItem::new("Autocomplete max items", "autocomplete-max-visible"),
        SelectItem::new(
            format!("Clear on shrink  [{}]", s(clear_shrink)),
            "clear-on-shrink",
        ),
        SelectItem::new(
            format!("Terminal progress  [{}]", s(term_prog)),
            "terminal-progress",
        ),
        SelectItem::new("Steering mode", "steering-mode"),
        SelectItem::new("Follow-up mode", "follow-up-mode"),
        SelectItem::new("Transport", "transport"),
        SelectItem::new("HTTP idle timeout", "http-idle-timeout"),
        SelectItem::new(
            format!("Show thinking  [{}]", s(show_thinking)),
            "hide-thinking",
        ),
        SelectItem::new(
            format!("Show tool calls  [{}]", s(show_tool_calls)),
            "show-tool-calls",
        ),
        SelectItem::new(
            format!("Collapse changelog  [{}]", s(collapse)),
            "collapse-changelog",
        ),
        SelectItem::new(format!("Quiet startup  [{}]", s(quiet)), "quiet-startup"),
        SelectItem::new(
            format!("Install telemetry  [{}]", s(telemetry)),
            "install-telemetry",
        ),
        SelectItem::new("Double-escape action", "double-escape-action"),
        SelectItem::new("Tree filter mode", "tree-filter-mode"),
        SelectItem::new("Warnings", "warnings"),
        SelectItem::new("Thinking level", "thinking"),
        SelectItem::new("Theme", "theme"),
        SelectItem::new("Models", "models"),
    ];
    let select = SelectList::new("Settings", items);
    ctx.tui.start_selection(select);
}

/// Handle /connect slash command
pub(crate) fn handle_login_command(ctx: &mut TuiContext) {
    ctx.pending_command = Some("connect".to_string());
    let items = vec![
        SelectItem::new("Use an API key", "apikey").with_description("Sign in using an API key"),
        SelectItem::new("Use a subscription", "subscription")
            .with_description("Sign in with a subscription"),
    ];
    let select = SelectList::new("Select authentication method", items);
    ctx.tui.start_selection(select);
}

/// Handle /unconnect slash command
pub(crate) fn handle_logout_command(ctx: &mut TuiContext) {
    let providers = ctx.auth.list_providers();
    if providers.is_empty() {
        ctx.tui
            .chat
            .add_system_message("No stored credentials to remove.");
    } else {
        ctx.pending_command = Some("unconnect".to_string());
        let items: Vec<SelectItem> = providers
            .iter()
            .map(|p| {
                SelectItem::new(p.clone(), p.clone()).with_description("Remove stored credential")
            })
            .collect();
        let select = SelectList::new("Select provider to disconnect from", items);
        ctx.tui.start_selection(select);
    }
}
