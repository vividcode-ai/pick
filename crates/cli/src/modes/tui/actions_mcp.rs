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

    // Get disabled server names
    let disabled = ctx
        .disabled_mcp_servers
        .lock()
        .map(|d| d.clone())
        .unwrap_or_default();

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
pub(crate) async fn show_mcp_server_detail(ctx: &mut TuiContext, server_name: &str) {
    ctx.pending_command = Some(format!("mcp-server:{}", server_name));

    // Get server info
    let info = get_server_info(ctx, server_name).await;

    // Show info as system messages
    let disabled = ctx
        .disabled_mcp_servers
        .lock()
        .map(|d| d.contains(&server_name.to_string()))
        .unwrap_or(false);

    ctx.tui.chat.add_system_message(&format!(
        "\x1b[1m=== MCP Server: {} ===\x1b[0m",
        server_name
    ));

    let status_text = if disabled {
        "\x1b[31mDisabled\x1b[0m"
    } else if info.as_ref().map(|i| i.is_connected).unwrap_or(false) {
        "\x1b[32mConnected\x1b[0m"
    } else {
        "\x1b[33mDisconnected\x1b[0m"
    };
    ctx.tui
        .chat
        .add_system_message(&format!("Status: {}", status_text));

    if let Some(info) = &info {
        ctx.tui
            .chat
            .add_system_message(&format!("Transport: \x1b[2m{}\x1b[0m", info.transport));

        // Show command or URL
        if let Some(cmd) = &info.command {
            let args_str = info.args.as_ref().map(|a| a.join(" ")).unwrap_or_default();
            ctx.tui
                .chat
                .add_system_message(&format!("Command: \x1b[2m{} {}\x1b[0m", cmd, args_str));
        } else if let Some(url) = &info.url {
            ctx.tui
                .chat
                .add_system_message(&format!("URL: \x1b[2m{}\x1b[0m", url));
        }

        // Config source
        let source = get_config_source(server_name);
        ctx.tui
            .chat
            .add_system_message(&format!("Config: \x1b[2m{}\x1b[0m", source));

        // Capabilities
        let caps = format!(
            "Tools: \x1b[1m{}\x1b[0m, Prompts: \x1b[1m{}\x1b[0m, Resources: \x1b[1m{}\x1b[0m",
            info.tool_count, info.prompt_count, info.resource_count
        );
        ctx.tui
            .chat
            .add_system_message(&format!("Capabilities: {}", caps));
    } else {
        ctx.tui
            .chat
            .add_system_message("No connection info available.");
    }

    // Build action items
    let mut action_items: Vec<SelectItem> = Vec::new();

    if disabled {
        action_items.push(SelectItem::new("Enable server", "enable"));
    } else if info.as_ref().map(|i| i.is_connected).unwrap_or(false) {
        action_items.push(SelectItem::new("View capabilities", "caps"));
        action_items.push(SelectItem::new("Disconnect server", "disconnect"));
        action_items.push(SelectItem::new("Disable server", "disable"));
    } else {
        // Disconnected but not disabled — can connect
        action_items.push(SelectItem::new("Connect server", "connect"));
    }
    action_items.push(SelectItem::new("Back to list", "back"));

    let select = SelectList::new("Actions", action_items);
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
        }
        "connect" => {
            connect_server(ctx, &server_name).await;
        }
        "disconnect" => {
            disconnect_server(ctx, &server_name).await;
        }
        "disable" => {
            disable_server(ctx, &server_name).await;
        }
        "enable" => {
            enable_server(ctx, &server_name).await;
        }
        "back" => {
            show_mcp_server_list(ctx).await;
            return;
        }
        _ => {
            ctx.tui
                .chat
                .add_system_message(&format!("Unknown action: {}", action));
        }
    }

    // Re-show server detail (or list) after action
    show_mcp_server_detail(ctx, &server_name).await;
}

// ──── Level 3: Capabilities ─────────────────────────────────────────────────

/// Show capabilities type selection (Level 3).
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
        items.push(SelectItem::new(
            format!("Tools ({})", info.tool_count),
            "tools",
        ));
    }
    if info.prompt_count > 0 {
        items.push(SelectItem::new(
            format!("Prompts ({})", info.prompt_count),
            "prompts",
        ));
    }
    if info.resource_count > 0 {
        items.push(SelectItem::new(
            format!("Resources ({})", info.resource_count),
            "resources",
        ));
    }
    items.push(SelectItem::new("Back", "back"));

    let select = SelectList::new(format!("Capabilities — {}", server_name), items);
    ctx.tui.start_selection(select);
    ctx.tui.finalize_turn();
}

/// Handle capability type selection.
pub(crate) async fn handle_mcp_capabilities_selection(ctx: &mut TuiContext, capability_type: &str) {
    // Parse server name from pending_command
    let cmd = ctx.pending_command.clone().unwrap_or_default();
    let server_name = cmd.strip_prefix("mcp-caps:").unwrap_or("").to_string();

    if capability_type == "back" {
        show_mcp_server_detail(ctx, &server_name).await;
        return;
    }

    let info = get_server_info(ctx, &server_name).await;
    let Some(info) = info else {
        show_mcp_server_detail(ctx, &server_name).await;
        return;
    };

    match capability_type {
        "tools" => {
            if info.tool_names.is_empty() {
                ctx.tui
                    .chat
                    .add_system_message(&format!("No tools for server '{}'.", server_name));
            } else {
                ctx.tui.chat.add_system_message(&format!(
                    "\x1b[1mTools ({}) — {}:\x1b[0m",
                    info.tool_count, server_name
                ));
                for name in &info.tool_names {
                    ctx.tui
                        .chat
                        .add_system_message(&format!("  \x1b[2m\u{2022}\x1b[0m {}", name));
                }
            }
        }
        "prompts" => {
            if info.prompt_names.is_empty() {
                ctx.tui
                    .chat
                    .add_system_message(&format!("No prompts for server '{}'.", server_name));
            } else {
                ctx.tui.chat.add_system_message(&format!(
                    "\x1b[1mPrompts ({}) — {}:\x1b[0m",
                    info.prompt_count, server_name
                ));
                for name in &info.prompt_names {
                    ctx.tui
                        .chat
                        .add_system_message(&format!("  \x1b[2m\u{2022}\x1b[0m {}", name));
                }
            }
        }
        "resources" => {
            if info.resource_names.is_empty() {
                ctx.tui
                    .chat
                    .add_system_message(&format!("No resources for server '{}'.", server_name));
            } else {
                ctx.tui.chat.add_system_message(&format!(
                    "\x1b[1mResources ({}) — {}:\x1b[0m",
                    info.resource_count, server_name
                ));
                for name in &info.resource_names {
                    ctx.tui
                        .chat
                        .add_system_message(&format!("  \x1b[2m\u{2022}\x1b[0m {}", name));
                }
            }
        }
        _ => {}
    }

    // Show capabilities selector again
    show_mcp_capabilities(ctx, &server_name).await;
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

    // Find the config for this server
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

    // Remove from disabled list if present
    if let Ok(mut disabled) = ctx.disabled_mcp_servers.lock() {
        disabled.retain(|n| n != server_name);
    }

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

async fn disconnect_server(ctx: &mut TuiContext, server_name: &str) {
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
            ctx.tui.chat.add_system_message(&format!(
                "\x1b[33mDisconnected MCP server '{}' ({} tool(s) removed)\x1b[0m",
                server_name,
                removed_tools.len()
            ));
        }
        Err(e) => {
            ctx.tui
                .chat
                .add_system_message(&format!("\x1b[31mError: {}\x1b[0m", e));
        }
    }
}

async fn disable_server(ctx: &mut TuiContext, server_name: &str) {
    // Add to disabled list
    if let Ok(mut disabled) = ctx.disabled_mcp_servers.lock() {
        if !disabled.contains(&server_name.to_string()) {
            disabled.push(server_name.to_string());
        }
    }

    // Disconnect if connected
    disconnect_server(ctx, server_name).await;

    ctx.tui.chat.add_system_message(&format!(
        "Server '{}' \x1b[31mdisabled\x1b[0m (session-only, re-enable from this menu).",
        server_name
    ));
}

async fn enable_server(ctx: &mut TuiContext, server_name: &str) {
    // Remove from disabled list
    if let Ok(mut disabled) = ctx.disabled_mcp_servers.lock() {
        disabled.retain(|n| n != server_name);
    }

    ctx.tui
        .chat
        .add_system_message(&format!("Server '{}' \x1b[32menabled\x1b[0m.", server_name));

    // Connect the server
    connect_server(ctx, server_name).await;
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
