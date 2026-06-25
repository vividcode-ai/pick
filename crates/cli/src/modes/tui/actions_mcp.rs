use std::sync::atomic::Ordering;

use pick_mcp::ConnectedServerInfo;
use pick_tui::components::select::{SelectItem, SelectList};

use super::context::TuiContext;
use super::init;

// ──── Level 1: MCP Server List ──────────────────────────────────────────────

/// Show the interactive MCP server list (Level 1).
pub(crate) async fn show_mcp_server_list(ctx: &mut TuiContext) {
    ctx.pending_command = Some("mcp".to_string());

    // Load settings to get configured MCP servers
    let cwd = std::env::current_dir().unwrap_or_default();
    let sm = crate::core::settings::SettingsManager::load(&cwd);
    let mcp_configs = sm
        .get()
        .mcp_servers
        .clone()
        .map(|servers| pick_mcp::parse_mcp_configs_from_value(&serde_json::json!(servers)))
        .unwrap_or_default();

    // Get all servers info (connected + configured but disconnected)
    let all_servers = ctx.mcp_manager.get_all_servers_info(&mcp_configs).await;

    // Get disabled server names from settings
    let disabled = sm.get_disabled_mcp_servers();

    if all_servers.is_empty() {
        ctx.tui.chat.add_system_message(
            "No MCP servers configured. Add servers in settings (`mcp_servers`).",
        );
        ctx.pending_command = None;
        ctx.tui.finalize_turn();
        return;
    }

    let items: Vec<SelectItem> = all_servers
        .iter()
        .map(|srv| {
            let status = if disabled.contains(&srv.name) {
                "disabled"
            } else if srv.is_connected {
                "connected"
            } else {
                "disconnected"
            };
            let tool_info = format!("  {} tool(s)", srv.tool_count);
            let label = format!("{}  [{}]{}", srv.name, status, tool_info);
            SelectItem::new(label, srv.name.clone()).with_description(status)
        })
        .collect();

    let select = SelectList::new("MCP Servers", items);
    ctx.tui.start_selection(select);
    ctx.tui.finalize_turn();
}

// ──── Level 2: Server Detail / Actions ──────────────────────────────────────

/// Handle user selecting a server from Level 1 → show detail + actions (Level 2).
pub(crate) async fn handle_mcp_server_selected(ctx: &mut TuiContext, server_name: &str) {
    show_mcp_server_detail(ctx, server_name).await;
}

/// Show server detail info + action SelectList (Level 2).
/// Info is shown in the popup via `info_lines`, not as chat messages.
pub(crate) async fn show_mcp_server_detail(ctx: &mut TuiContext, server_name: &str) {
    ctx.pending_command = Some(format!("mcp-server:{}", server_name));

    // Get server info
    let info = get_server_info(ctx, server_name).await;

    // Check disabled state from settings
    let cwd = std::env::current_dir().unwrap_or_default();
    let sm = crate::core::settings::SettingsManager::load(&cwd);
    let disabled_servers = sm.get_disabled_mcp_servers();
    let is_disabled = disabled_servers.contains(&server_name.to_string());
    let is_connected = info.as_ref().map(|i| i.is_connected).unwrap_or(false);

    // Build info lines for popup
    let mut info_lines: Vec<String> = Vec::new();

    let status_text = if is_disabled {
        "\x1b[31mDisabled\x1b[0m".to_string()
    } else if is_connected {
        "\x1b[32mConnected\x1b[0m".to_string()
    } else {
        "\x1b[33mDisconnected\x1b[0m".to_string()
    };
    info_lines.push(format!("Status: {}", status_text));

    if let Some(info) = &info {
        info_lines.push(format!("Transport: {}", info.transport));

        // Show command or URL
        if let Some(cmd) = &info.command {
            let args_str = info.args.as_ref().map(|a| a.join(" ")).unwrap_or_default();
            info_lines.push(format!("Command: {} {}", cmd, args_str));
        } else if let Some(url) = &info.url {
            info_lines.push(format!("URL: {}", url));
        }

        // Config source
        let source = get_config_source(server_name);
        info_lines.push(format!("Config: {}", source));

        // Capabilities
        info_lines.push(format!(
            "Capabilities: Tools: {}, Prompts: {}, Resources: {}",
            info.tool_count, info.prompt_count, info.resource_count
        ));
    } else {
        info_lines.push("No connection info available.".to_string());
    }

    // Build action items — exactly 3
    let mut action_items: Vec<SelectItem> = Vec::new();

    // Item 1: View capability (only show one based on what the server primarily provides)
    if is_connected {
        if let Some(info) = &info {
            if info.tool_count > 0 {
                action_items.push(SelectItem::new(
                    format!("View tools ({})", info.tool_count),
                    "caps",
                ));
            } else if info.prompt_count > 0 {
                action_items.push(SelectItem::new(
                    format!("View prompts ({})", info.prompt_count),
                    "caps",
                ));
            } else if info.resource_count > 0 {
                action_items.push(SelectItem::new(
                    format!("View resources ({})", info.resource_count),
                    "caps",
                ));
            }
        }
    }

    // Item 2: Connect / Reconnect / (disabled shows nothing)
    if is_disabled {
        // No connect action — disabled servers don't start
    } else if is_connected {
        action_items.push(SelectItem::new("Reconnect", "reconnect"));
    } else {
        action_items.push(SelectItem::new("Connect", "connect"));
    }

    // Item 3: Disable / Enable (always available)
    if is_disabled {
        action_items.push(SelectItem::new("Enable", "enable"));
    } else {
        action_items.push(SelectItem::new("Disable", "disable"));
    }

    let select = SelectList::new(format!("MCP Server: {}", server_name), action_items)
        .with_info_lines(info_lines);
    ctx.tui.start_selection(select);
    ctx.tui.finalize_turn();
}

// ──── Level 2 Action Handlers ───────────────────────────────────────────────

/// Handle action selected from Level 2 (server detail).
pub(crate) async fn handle_mcp_server_action(ctx: &mut TuiContext, action: &str) {
    // Parse server name from pending_command
    let cmd = ctx.pending_command.clone().unwrap_or_default();
    let server_name = cmd.strip_prefix("mcp-server:").unwrap_or("").to_string();

    match action {
        "caps" => {
            show_mcp_capabilities(ctx, &server_name).await;
            return; // don't re-show detail, stay in capabilities
        }
        "connect" => {
            connect_server(ctx, &server_name).await;
        }
        "reconnect" => {
            // Disconnect first
            let removed = ctx.mcp_manager.disconnect_server(&server_name).await;
            if let Ok(ref removed_tools) = removed {
                if let Ok(mut locked) = ctx.all_tools.write() {
                    for tool_name in removed_tools {
                        locked.retain(|t| t.name != *tool_name);
                    }
                }
            }
            // Then reconnect
            connect_server(ctx, &server_name).await;
        }
        "disable" => {
            disable_server(ctx, &server_name).await;
        }
        "enable" => {
            enable_server(ctx, &server_name).await;
            return; // show_mcp_server_detail will be called by enable_server
        }
        _ => {
            ctx.tui
                .chat
                .add_system_message(&format!("Unknown action: {}", action));
        }
    }

    // Re-show server detail after action
    show_mcp_server_detail(ctx, &server_name).await;
}

// ──── Level 3: Capabilities ─────────────────────────────────────────────────

/// Show capability details for a server (Level 3).
async fn show_mcp_capabilities(ctx: &mut TuiContext, server_name: &str) {
    ctx.pending_command = Some(format!("mcp-caps:{}", server_name));

    let info = get_server_info(ctx, server_name).await;
    let Some(info) = info else {
        ctx.tui
            .chat
            .add_system_message("Server not connected — no capabilities available.");
        show_mcp_server_detail(ctx, server_name).await;
        return;
    };

    let mut items = Vec::new();

    if info.tool_count > 0 {
        for name in &info.tool_names {
            items.push(SelectItem::new(name.clone(), name.clone()));
        }
    }
    if info.prompt_count > 0 {
        for name in &info.prompt_names {
            items.push(SelectItem::new(name.clone(), name.clone()));
        }
    }
    if info.resource_count > 0 {
        for name in &info.resource_names {
            items.push(SelectItem::new(name.clone(), name.clone()));
        }
    }

    items.push(SelectItem::new("Back", "back"));

    let total = info.tool_count + info.prompt_count + info.resource_count;
    let title = format!("Capabilities: {} — {} items", server_name, total);

    let select = SelectList::new(title, items);
    ctx.tui.start_selection(select);
    ctx.tui.finalize_turn();
}

/// Handle capability item selection (or "back").
pub(crate) async fn handle_mcp_capabilities_selection(ctx: &mut TuiContext, value: &str) {
    let cmd = ctx.pending_command.clone().unwrap_or_default();
    let server_name = cmd.strip_prefix("mcp-caps:").unwrap_or("").to_string();

    if value == "back" {
        show_mcp_server_detail(ctx, &server_name).await;
        return;
    }

    // Just showing the capability name — go back to detail
    show_mcp_server_detail(ctx, &server_name).await;
}

// ──── Server Actions ────────────────────────────────────────────────────────

async fn connect_server(ctx: &mut TuiContext, server_name: &str) {
    // Load config from settings
    let cwd = std::env::current_dir().unwrap_or_default();
    let sm = crate::core::settings::SettingsManager::load(&cwd);
    let mcp_configs = sm
        .get()
        .mcp_servers
        .clone()
        .map(|servers| pick_mcp::parse_mcp_configs_from_value(&serde_json::json!(servers)))
        .unwrap_or_default();

    let config = mcp_configs.into_iter().find(|c| c.name == server_name);
    let config = match config {
        Some(c) => c,
        None => {
            ctx.tui
                .chat
                .add_system_message(&format!(
                    "Server '{}' not found in settings. Use \x1b[1m/mcp connect {} --command ...\x1b[0m to connect manually.",
                    server_name, server_name
                ));
            return;
        }
    };

    match ctx.mcp_manager.connect_server(config).await {
        Ok(new_tools) => {
            let count = new_tools.len();
            if let Ok(mut locked) = ctx.all_tools.write() {
                locked.extend(new_tools);
            }
            ctx.tools = init::refilter_tools(
                &ctx.all_tools,
                &ctx.agent_mode,
                &ctx.session_manager,
                &ctx.mcp_manager,
                ctx.mcp_enabled.load(Ordering::Relaxed),
            )
            .await;
            ctx.system_prompt = init::rebuild_system_prompt(
                &ctx.tools,
                &ctx.resource_loader,
                &ctx.cwd,
                &ctx.provider,
                &ctx.model_id,
                ctx.args.system_prompt.as_deref(),
                &ctx.args.append_system_prompt,
                Some(&ctx.agent_mode),
            );
            ctx.tui.chat.add_system_message(&format!(
                "\x1b[32mConnected to MCP server '{}' ({} tool(s))\x1b[0m",
                server_name, count
            ));
        }
        Err(e) => {
            ctx.tui.chat.add_system_message(&format!(
                "\x1b[31mFailed to connect MCP server '{}': {}\x1b[0m",
                server_name, e
            ));
        }
    }
}

async fn disable_server(ctx: &mut TuiContext, server_name: &str) {
    // Disconnect if connected
    if ctx.mcp_manager.is_server_connected(server_name).await {
        match ctx.mcp_manager.disconnect_server(server_name).await {
            Ok(removed_tools) => {
                if let Ok(mut locked) = ctx.all_tools.write() {
                    for tool_name in &removed_tools {
                        locked.retain(|t| t.name != *tool_name);
                    }
                }
                ctx.tools = init::refilter_tools(
                    &ctx.all_tools,
                    &ctx.agent_mode,
                    &ctx.session_manager,
                    &ctx.mcp_manager,
                    ctx.mcp_enabled.load(Ordering::Relaxed),
                )
                .await;
                ctx.system_prompt = init::rebuild_system_prompt(
                    &ctx.tools,
                    &ctx.resource_loader,
                    &ctx.cwd,
                    &ctx.provider,
                    &ctx.model_id,
                    ctx.args.system_prompt.as_deref(),
                    &ctx.args.append_system_prompt,
                    Some(&ctx.agent_mode),
                );
            }
            Err(e) => {
                ctx.tui
                    .chat
                    .add_system_message(&format!("\x1b[31mError disconnecting: {}\x1b[0m", e));
            }
        }
    }

    // Save disabled state to settings
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut sm = crate::core::settings::SettingsManager::load(&cwd);
    let mut disabled = sm.get_disabled_mcp_servers();
    if !disabled.contains(&server_name.to_string()) {
        disabled.push(server_name.to_string());
    }
    let mut update = crate::core::settings::Settings::default();
    update.disabled_mcp_servers = Some(disabled);
    if let Err(e) = sm.set_global(update) {
        ctx.tui
            .chat
            .add_system_message(&format!("\x1b[31mFailed to save: {}\x1b[0m", e));
    }

    // Update runtime disabled list
    if let Ok(mut disabled_runtime) = ctx.disabled_mcp_servers.lock() {
        if !disabled_runtime.contains(&server_name.to_string()) {
            disabled_runtime.push(server_name.to_string());
        }
    }

    ctx.tui.chat.add_system_message(&format!(
        "Server '{}' \x1b[31mdisabled\x1b[0m — will not start on next launch.",
        server_name
    ));
}

async fn enable_server(ctx: &mut TuiContext, server_name: &str) {
    // Remove from disabled state
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut sm = crate::core::settings::SettingsManager::load(&cwd);
    let mut disabled = sm.get_disabled_mcp_servers();
    disabled.retain(|n| n != server_name);
    let mut update = crate::core::settings::Settings::default();
    update.disabled_mcp_servers = Some(disabled);
    if let Err(e) = sm.set_global(update) {
        ctx.tui
            .chat
            .add_system_message(&format!("\x1b[31mFailed to save: {}\x1b[0m", e));
    }

    // Update runtime disabled list
    if let Ok(mut disabled_runtime) = ctx.disabled_mcp_servers.lock() {
        disabled_runtime.retain(|n| n != server_name);
    }

    ctx.tui
        .chat
        .add_system_message(&format!("Server '{}' \x1b[32menabled\x1b[0m.", server_name));

    // Now connect
    connect_server(ctx, server_name).await;
    show_mcp_server_detail(ctx, server_name).await;
}

// ──── Helpers ───────────────────────────────────────────────────────────────

/// Get server info from the MCP manager (connected servers only).
async fn get_server_info(ctx: &TuiContext, server_name: &str) -> Option<ConnectedServerInfo> {
    let info = ctx.mcp_manager.list_connections().await;
    info.into_iter().find(|srv| srv.name == server_name)
}

/// Determine where an MCP server config comes from (global vs project settings).
fn get_config_source(server_name: &str) -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    let sm = crate::core::settings::SettingsManager::load(&cwd);

    let in_global = sm
        .get_global()
        .mcp_servers
        .as_ref()
        .map(|s| s.contains_key(server_name))
        .unwrap_or(false);
    let in_project = sm
        .get_project()
        .mcp_servers
        .as_ref()
        .map(|s| s.contains_key(server_name))
        .unwrap_or(false);

    match (in_global, in_project) {
        (true, true) => "Global + Project settings".to_string(),
        (true, false) => "Global settings (~/.pick/settings.json)".to_string(),
        (false, true) => "Project settings (.pick/settings.json)".to_string(),
        (false, false) => "Runtime (not in settings)".to_string(),
    }
}
