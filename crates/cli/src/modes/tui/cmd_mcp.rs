use super::context::TuiContext;
use super::init;

/// Handle /mcp slash command
pub(crate) async fn handle_mcp(ctx: &mut TuiContext, args: &[String]) {
    if args.is_empty() {
        ctx.tui
            .chat
            .add_system_message("\x1b[1mMCP Server Management\x1b[0m");
        ctx.tui.chat.add_system_message(
            "  \x1b[2m/mcp list\x1b[0m              List connected MCP servers",
        );
        ctx.tui.chat.add_system_message(
            "  \x1b[2m/mcp connect <name> ...\x1b[0m   Connect a new MCP server",
        );
        ctx.tui.chat.add_system_message(
            "  \x1b[2m/mcp disconnect <name>\x1b[0m    Disconnect an MCP server",
        );
        return;
    }

    match args[0].as_str() {
        "list" => {
            let info = ctx.mcp_manager.list_connections().await;
            if info.is_empty() {
                ctx.tui.chat.add_system_message("No MCP servers connected.");
            } else {
                ctx.tui.chat.add_system_message(&format!(
                    "\x1b[1m{} connected MCP server(s):\x1b[0m",
                    info.len()
                ));
                for srv in &info {
                    let names = srv.tool_names.join(", ");
                    ctx.tui.chat.add_system_message(&format!(
                        "  \x1b[1m{}\x1b[0m [{}] — {} tool(s): {}",
                        srv.name, srv.transport, srv.tool_count, names
                    ));
                }
            }
        }
        "connect" => {
            if args.len() < 2 {
                ctx.tui
                    .chat
                    .add_system_message("Usage: /mcp connect <name> --command <cmd> [--args ...]");
            } else {
                let server_name = args[1].to_string();
                let mut command = None::<String>;
                let mut args_list = Vec::<String>::new();
                let mut i = 2;
                while i < args.len() {
                    match args[i].as_str() {
                        "--command" => {
                            i += 1;
                            if i < args.len() {
                                command = Some(args[i].to_string());
                            }
                        }
                        "--args" => {
                            i += 1;
                            while i < args.len() && !args[i].starts_with("--") {
                                args_list.push(args[i].to_string());
                                i += 1;
                            }
                            continue;
                        }
                        _ => {}
                    }
                    i += 1;
                }

                if let Some(cmd) = command {
                    let config = pick_mcp::McpServerConfig {
                        name: server_name,
                        command: Some(cmd),
                        args: Some(args_list),
                        env: None,
                        url: None,
                        tool_name_prefix: None,
                        auth: None,
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
                                ctx.mcp_enabled.load(std::sync::atomic::Ordering::Relaxed),
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
                                "\x1b[32mConnected to MCP server ({} tool(s))\x1b[0m",
                                count
                            ));
                        }
                        Err(e) => {
                            ctx.tui.chat.add_system_message(&format!(
                                "\x1b[31mFailed to connect MCP server: {}\x1b[0m",
                                e
                            ));
                        }
                    }
                } else {
                    ctx.tui
                        .chat
                        .add_system_message("Error: --command is required");
                }
            }
        }
        "disconnect" => {
            if args.len() < 2 {
                ctx.tui
                    .chat
                    .add_system_message("Usage: /mcp disconnect <name>");
            } else {
                let name = args[1].to_string();
                match ctx.mcp_manager.disconnect_server(&name).await {
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
                            ctx.mcp_enabled.load(std::sync::atomic::Ordering::Relaxed),
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
                            name,
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
        }
        _ => {
            ctx.tui.chat.add_system_message(&format!(
                "Unknown mcp subcommand: \x1b[1m{}\x1b[0m. Use \x1b[1m/mcp\x1b[0m for help.",
                args[0]
            ));
        }
    }
}
