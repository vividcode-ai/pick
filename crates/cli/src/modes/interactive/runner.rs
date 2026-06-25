use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};

use crate::args::Args;
use crate::core::agent_mode::{AgentMode, PLAN_MODE_REMINDER};
use crate::core::auth_storage::AuthStorage;
use crate::core::compaction::compaction::{
    CompactionSettings, compact, prepare_compaction, should_compact,
};
use crate::core::resource_loader::{ResourceLoader, ResourceLoaderOptions};
use crate::core::system_prompt::build_system_prompt_with_defaults_and_mode;
use crate::core::tools::render_tool_call;
use crate::core::tools::render_utils::{ToolRenderContext, ToolTheme};
use pick_agent::core::agent_loop::AgentLoopConfig;
use pick_agent::core::events::AgentEvent;
use pick_agent::core::state::{AgentTool, QuestionPrompt, ThinkingLevel};
use pick_agent::extensions::runner::ExtensionRunner;
use pick_agent::extensions::types::{
    ExtensionEvent, SessionBeforeCompactEvent, SessionCompactEvent,
};
use pick_agent::session::{CompactionEntry, SessionEntry, SessionEntryKind, SessionManager};
use pick_ai::models::get_model;
use pick_ai::types::{ContentBlock, Message, UserMessage};
use pick_mcp::{McpManager, McpServerConfig};

use super::oauth::handle_oauth_login;

pub async fn run_interactive_mode(
    args: Args,
    all_tools: Arc<RwLock<Vec<AgentTool>>>,
    auth: Arc<AuthStorage>,
    mut session_manager: SessionManager,
    initial_messages: Vec<Message>,
    extension_runner: Option<Arc<ExtensionRunner>>,
    mut agent_mode: AgentMode,
    agent_registry: Arc<pick_agent::agent_registry::AgentRegistry>,
    mcp_manager: Arc<McpManager>,
    mcp_done_rx: tokio::sync::watch::Receiver<bool>,
    mcp_cancelled: Arc<std::sync::atomic::AtomicBool>,
    permission_manager: Arc<pick_agent::permission::manager::PermissionManager>,
    platform_sandbox: Option<std::sync::Arc<dyn pick_agent::permission::sandbox::Sandbox>>,
    sandbox_enabled: Arc<AtomicBool>,
) {
    let provider = args.provider.as_deref().unwrap_or("anthropic");
    let model_id = args.model.as_deref().unwrap_or("claude-sonnet-4-20250514");
    let model = get_model(provider, model_id);

    let model = match model {
        Some(m) => m,
        None => {
            eprintln!(
                "Error: model '{}' not found for provider '{}'",
                model_id, provider
            );
            std::process::exit(1);
        }
    };

    let thinking_level = match args.thinking.as_deref() {
        Some("off") => ThinkingLevel::Off,
        Some("low") => ThinkingLevel::Low,
        Some("medium") => ThinkingLevel::Medium,
        Some("high") => ThinkingLevel::High,
        _ => {
            if model.reasoning {
                ThinkingLevel::High
            } else {
                ThinkingLevel::Off
            }
        }
    };

    let mut tools = {
        let mode_ruleset = agent_mode.ruleset();
        let locked = all_tools.read().unwrap();
        if args.no_tools {
            Vec::new()
        } else {
            pick_agent::tools::filter_goal_tools(
                pick_agent::permission::disabled::filter_tools(locked.clone(), &[&mode_ruleset]),
                session_manager.goal_manager(),
            )
        }
    };
    let mut all_messages = initial_messages;

    let cwd = std::env::current_dir().unwrap_or_default();
    let agent_dir = crate::config::get_agent_dir();
    let mut resource_loader = ResourceLoader::new(cwd.clone(), agent_dir);
    resource_loader
        .reload_with_options(
            &args.extensions,
            &ResourceLoaderOptions {
                no_skills: args.no_skills,
                no_themes: args.no_themes,
                no_context_files: args.no_context_files,
                theme_paths: args.themes.iter().map(std::path::PathBuf::from).collect(),
            },
        )
        .await;

    let custom_prompt = args
        .system_prompt
        .as_deref()
        .or_else(|| resource_loader.system_prompt());
    let loader_append = resource_loader.append_system_prompt().join("\n");
    let mut append_parts: Vec<String> = Vec::new();
    append_parts.extend(args.append_system_prompt.clone());
    if !loader_append.is_empty() {
        append_parts.push(loader_append);
    }
    let append_text = if append_parts.is_empty() {
        format!("Provider: {}  Model: {}", provider, model_id)
    } else {
        format!(
            "{}\nProvider: {}  Model: {}",
            append_parts.join("\n"),
            provider,
            model_id
        )
    };
    let mut system_prompt = build_system_prompt_with_defaults_and_mode(
        &tools,
        resource_loader.skills(),
        resource_loader.agents_files(),
        custom_prompt,
        Some(&append_text),
        &cwd,
        Some(&agent_mode),
    );

    let mode_shared = Arc::new(Mutex::new(agent_mode));

    let cwd = std::env::current_dir().unwrap_or_default();
    let version = crate::config::VERSION;
    println!("\x1b[1m Pick v{}\x1b[0m", version);
    println!("\x1b[2m Ctrl+C clear/exit · / commands · /help more\x1b[0m");
    println!();
    println!(" Type /help to show commands and loaded resources.");
    println!();

    if let Some(ref runner) = extension_runner {
        let ext_paths = runner.get_extension_paths();
        if !ext_paths.is_empty() {
            println!("\x1b[1m[Extensions]\x1b[0m");
            for p in &ext_paths {
                let name = std::path::Path::new(p)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(p);
                println!("  {}", name);
            }
            println!();
        }
    }

    let separator = "─".repeat(80.min(cwd.to_string_lossy().len().max(30)));
    println!("\x1b[2m{}\x1b[0m", separator);
    println!();
    println!("{}", cwd.display());
    println!();

    loop {
        print!("> ");
        use std::io::Write;
        std::io::stdout().flush().ok();

        let mut input = String::new();
        match std::io::stdin().read_line(&mut input) {
            Ok(0) => break,
            Err(_) => break,
            Ok(_) => {
                let mut input = input.trim().to_string();
                if input.is_empty() {
                    if mcp_done_rx.has_changed().unwrap_or(false) && *mcp_done_rx.borrow() {
                        println!("\x1b[32mMCP tools loaded in background\x1b[0m");
                    }
                    continue;
                }

                match input.as_str() {
                    "/quit" | "/exit" | "/q" => {
                        mcp_cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
                        mcp_manager.shutdown().await;
                        break;
                    }
                    "/help" | "/h" => {
                        println!("Commands:");
                        println!("  /quit, /exit, /q  Exit");
                        println!("  /help, /h        Show this help");
                        println!("  /clear           Clear screen");
                        println!("  /model <name>    Show/set model");
                        println!("  /auth <cmd>      Manage credentials (set|remove|login|list)");
                        println!("  /connect         OAuth login flow");
                        println!("  /info             Show session info");
                        println!("  /session list     List recent sessions");
                        println!("  /compact           Compact conversation context");
                        println!("  /fork at <N>      Fork session at message N");
                        let sm = crate::core::settings::SettingsManager::load(&cwd);
                        if sm.get_enable_skill_commands() {
                            let skills = resource_loader.skills();
                            if !skills.is_empty() {
                                println!();
                                println!("Skills:");
                                for skill in skills {
                                    println!("  /skill:{}", skill.name);
                                }
                            }
                        }
                        continue;
                    }
                    "/clear" => {
                        print!("\x1b[2J\x1b[H");
                        std::io::stdout().flush().ok();
                        continue;
                    }
                    "/model" => {
                        println!("Current model: {} ({})", model_id, provider);
                        continue;
                    }
                    "/connect" => {
                        handle_oauth_login(&auth).await;
                        continue;
                    }
                    "/plan" | "/plan_enter" => {
                        switch_agent_mode(&mut agent_mode, AgentMode::Plan, &mut session_manager)
                            .await;
                        *mode_shared.lock().unwrap() = agent_mode;
                        tools = pick_agent::tools::filter_goal_tools(
                            pick_agent::permission::disabled::filter_tools(
                                all_tools.read().unwrap().clone(),
                                &[&agent_mode.ruleset()],
                            ),
                            session_manager.goal_manager(),
                        );
                        system_prompt = build_system_prompt_with_defaults_and_mode(
                            &tools,
                            resource_loader.skills(),
                            resource_loader.agents_files(),
                            custom_prompt,
                            Some(&append_text),
                            &cwd,
                            Some(&agent_mode),
                        );
                        println!("\x1b[36mSwitched to PLAN mode (read-only)\x1b[0m");
                        continue;
                    }
                    "/build" | "/plan_exit" => {
                        let was_plan = agent_mode == AgentMode::Plan;
                        switch_agent_mode(&mut agent_mode, AgentMode::Build, &mut session_manager)
                            .await;
                        *mode_shared.lock().unwrap() = agent_mode;
                        tools = pick_agent::tools::filter_goal_tools(
                            pick_agent::permission::disabled::filter_tools(
                                all_tools.read().unwrap().clone(),
                                &[&agent_mode.ruleset()],
                            ),
                            session_manager.goal_manager(),
                        );
                        system_prompt = build_system_prompt_with_defaults_and_mode(
                            &tools,
                            resource_loader.skills(),
                            resource_loader.agents_files(),
                            custom_prompt,
                            Some(&append_text),
                            &cwd,
                            Some(&agent_mode),
                        );
                        let msg = if was_plan {
                            AgentMode::build_switch_prompt()
                        } else {
                            "Switched to BUILD mode"
                        };
                        println!("\x1b[32m{}\x1b[0m", msg);

                        // Inject mode switch into conversation history so LLM is aware
                        if was_plan {
                            all_messages.push(Message::User(UserMessage::text(
                                AgentMode::build_switch_prompt(),
                            )));
                        }

                        continue;
                    }
                    "/info" => {
                        let n = all_messages.len();
                        let sid = session_manager
                            .header()
                            .map(|h| h.id.as_str())
                            .unwrap_or("(new)");
                        println!("Session ID: {}", sid);
                        println!("Messages:   {}", n);
                        println!("Provider:   {}", provider);
                        println!("Model:      {}", model_id);
                        println!("Mode:       {}", agent_mode);
                        continue;
                    }
                    cmd if cmd == "/goal" || cmd.starts_with("/goal ") => {
                        let goal_manager = session_manager.goal_manager();
                        let args = cmd.strip_prefix("/goal").map(|s| s.trim()).unwrap_or("");
                        if args.is_empty() {
                            match goal_manager.get() {
                                Some(goal) => {
                                    let remaining = goal_manager
                                        .remaining_tokens()
                                        .map(|r| format!(", remaining: {}", r))
                                        .unwrap_or_default();
                                    println!(
                                        "\x1b[1mGoal\x1b[0m  \x1b[36m{}\x1b[0m",
                                        goal.objective
                                    );
                                    println!(
                                        "  Status: {}  Tokens: {}{}",
                                        goal.status, goal.tokens_used, remaining
                                    );
                                }
                                None => {
                                    println!("Usage: \x1b[33m/goal <objective>\x1b[0m");
                                }
                            }
                        } else {
                            match args.to_ascii_lowercase().as_str() {
                                "clear" => {
                                    session_manager.clear_goal().await.ok();
                                    println!("\x1b[33mGoal cleared.\x1b[0m");
                                }
                                "pause" => match goal_manager.set_paused() {
                                    Ok(g) => {
                                        session_manager.persist_goal().await.ok();
                                        println!("\x1b[33mGoal paused.\x1b[0m  {}", g.objective);
                                    }
                                    Err(e) => eprintln!("Error: {}", e),
                                },
                                "resume" => match goal_manager.set_active() {
                                    Ok(g) => {
                                        session_manager.persist_goal().await.ok();
                                        println!("\x1b[32mGoal resumed.\x1b[0m  {}", g.objective);
                                    }
                                    Err(e) => eprintln!("Error: {}", e),
                                },
                                _ => {
                                    if goal_manager.get().is_some() {
                                        println!("A goal already exists. Use /goal clear first.");
                                    } else if let Err(e) =
                                        goal_manager.create(args.to_string(), None)
                                    {
                                        eprintln!("Error: {}", e);
                                    } else {
                                        session_manager.persist_goal().await.ok();
                                        println!("\x1b[32mGoal set:\x1b[0m  {}", args);
                                    }
                                }
                            }
                        }
                        continue;
                    }
                    _ => {}
                }

                if input == "/session list" {
                    let dir = crate::core::session_manager::get_default_session_dir(
                        &std::env::current_dir()
                            .unwrap_or_default()
                            .to_string_lossy(),
                        &crate::config::get_agent_dir().to_string_lossy(),
                    );
                    let sessions = crate::core::session_manager::list_sessions_from_dir(&dir).await;
                    if sessions.is_empty() {
                        println!("No sessions found.");
                    } else {
                        for (i, s) in sessions.iter().enumerate().take(10) {
                            let name = s.name.as_deref().unwrap_or("(unnamed)");
                            let modified = s.modified.format("%Y-%m-%d %H:%M");
                            println!(
                                "  {}. {} [{} msgs] {}",
                                i + 1,
                                name,
                                s.message_count,
                                modified
                            );
                        }
                    }
                    continue;
                }

                if input.starts_with("/compact") {
                    let custom_instructions = if input.len() > 9 {
                        Some(input[9..].trim().to_string())
                    } else {
                        None
                    };
                    let msg_count = all_messages.len();
                    if msg_count < 2 {
                        println!("Nothing to compact (need at least 2 messages).");
                    } else {
                        println!("Compacting {} messages...", msg_count);
                        let path_entries: Vec<serde_json::Value> = all_messages.iter().map(|msg| {
                            let id = uuid::Uuid::now_v7().to_string();
                            let message_val = match msg {
                                Message::User(u) => serde_json::json!({
                                    "role": "user",
                                    "content": u.content,
                                }),
                                Message::Assistant(a) => serde_json::json!({
                                    "role": "assistant",
                                    "content": a.content,
                                    "stopReason": format!("{:?}", a.stop_reason),
                                }),
                                Message::ToolResult(t) => serde_json::json!({
                                    "role": "toolResult",
                                    "content": t.content,
                                    "toolCallId": t.tool_call_id,
                                    "toolName": t.tool_name,
                                    "isError": t.is_error,
                                }),
                            };
                            serde_json::json!({"id": id, "type": "message", "message": message_val})
                        }).collect();

                        if let Some(ref runner) = extension_runner {
                            runner.emit(&ExtensionEvent::SessionBeforeCompact(
                                SessionBeforeCompactEvent {
                                    preparation: serde_json::json!({}),
                                    branch_entries: path_entries.clone(),
                                    custom_instructions: custom_instructions.clone(),
                                },
                            ));
                        }

                        let compact_settings = CompactionSettings::default();
                        match prepare_compaction(&path_entries, &compact_settings) {
                            Some(preparation) => {
                                let api_key =
                                    auth.get_api_key(provider, true).await.unwrap_or_default();
                                match compact(
                                    &preparation,
                                    &model,
                                    &api_key,
                                    None,
                                    custom_instructions.as_deref(),
                                    None,
                                )
                                .await
                                {
                                    Ok(result) => {
                                        let summary = result.summary;
                                        all_messages =
                                            vec![Message::User(UserMessage::text(format!(
                                                "[Compacted conversation summary]\n\n{}",
                                                summary
                                            )))];
                                        println!(
                                            "Compacted ({} msgs → 1, {} tokens before).",
                                            msg_count, result.tokens_before
                                        );
                                        let compact_entry = SessionEntry {
                                            id: uuid::Uuid::now_v7().to_string(),
                                            parent_id: None,
                                            timestamp: chrono::Utc::now().timestamp_millis(),
                                            kind: SessionEntryKind::Compaction(CompactionEntry {
                                                summary: summary.clone(),
                                                token_count: Some(result.tokens_before as u64),
                                            }),
                                        };
                                        if let Err(e) = session_manager.append(compact_entry).await
                                        {
                                            eprintln!(
                                                "Warning: failed to persist compaction entry: {}",
                                                e
                                            );
                                        }
                                        if let Some(ref runner) = extension_runner {
                                            runner.emit(&ExtensionEvent::SessionCompact(
                                                SessionCompactEvent {
                                                    compaction_entry: serde_json::json!({
                                                        "summary": summary,
                                                        "tokensBefore": result.tokens_before,
                                                    }),
                                                    from_extension: false,
                                                },
                                            ));
                                        }
                                    }
                                    Err(e) => {
                                        println!("Compaction failed: {}", e.message);
                                    }
                                }
                            }
                            None => {
                                println!("Compaction preparation failed.");
                            }
                        }
                    }
                    continue;
                }

                if let Some(rest) = input.strip_prefix("/fork at ") {
                    if let Ok(n) = rest.trim().parse::<usize>() {
                        if n > 0 && n <= all_messages.len() {
                            let idx = n - 1;
                            let cwd = std::env::current_dir().unwrap_or_default();
                            let session_dir = cwd.join(".pick").join("sessions");
                            match SessionManager::create(cwd, Some(session_dir)).await {
                                Ok(mut new_mgr) => {
                                    let fork_msgs: Vec<Message> = all_messages[..=idx].to_vec();
                                    for msg in &fork_msgs {
                                        if let Err(e) =
                                            new_mgr.append(SessionEntry::from(msg)).await
                                        {
                                            eprintln!("Warning: fork persist failed: {}", e);
                                        }
                                    }
                                    all_messages = fork_msgs;
                                    session_manager = new_mgr;
                                    println!(
                                        "Forked new session with {} messages.",
                                        all_messages.len()
                                    );
                                }
                                Err(e) => {
                                    println!("Fork failed: {}", e);
                                }
                            }
                        } else {
                            println!("Invalid message number. Use 1-{}", all_messages.len());
                        }
                    } else {
                        println!("Usage: /fork at <message_number>");
                    }
                    continue;
                }

                if let Some(model_name) = input.strip_prefix("/model ") {
                    let new_model = get_model(provider, model_name.trim());
                    match new_model {
                        Some(_) => println!("Model set to: {}", model_name.trim()),
                        None => println!("Model '{}' not found", model_name.trim()),
                    }
                    continue;
                }

                if let Some(rest) = input.strip_prefix("/auth ") {
                    let parts: Vec<&str> = rest.splitn(3, ' ').collect();
                    match parts[0] {
                        "set" if parts.len() >= 3 => {
                            auth.set_api_key(parts[1], parts[2]);
                            println!("Credential saved for provider '{}'", parts[1]);
                        }
                        "remove" if parts.len() >= 2 => {
                            auth.remove(parts[1]);
                            println!("Credential removed for provider '{}'", parts[1]);
                        }
                        "login" => {
                            handle_oauth_login(&auth).await;
                        }
                        "list" => {
                            let providers = auth.list_providers();
                            if providers.is_empty() {
                                println!("No stored credentials");
                            } else {
                                println!("Stored credentials for: {:?}", providers);
                            }
                        }
                        _ => {
                            println!("Usage:");
                            println!("  /auth set <provider> <key>   Save API key");
                            println!("  /auth remove <provider>     Remove API key");
                            println!("  /auth login                  OAuth login flow");
                            println!("  /auth list                   List providers");
                        }
                    }
                    continue;
                }

                if input.starts_with("/mcp") {
                    let args: Vec<&str> = input.split_whitespace().collect();
                    if args.len() < 2 {
                        println!("MCP Server Management:");
                        println!("  /mcp list                 List connected MCP servers");
                        println!("  /mcp connect <name> ...   Connect a new MCP server");
                        println!("  /mcp disconnect <name>    Disconnect an MCP server");
                    } else {
                        match args[1] {
                            "list" => {
                                let info = mcp_manager.list_connections().await;
                                if info.is_empty() {
                                    println!("No MCP servers connected.");
                                } else {
                                    println!("{} connected MCP server(s):", info.len());
                                    for srv in &info {
                                        let names = srv.tool_names.join(", ");
                                        println!(
                                            "  {} [{}] — {} tool(s): {}",
                                            srv.name, srv.transport, srv.tool_count, names
                                        );
                                    }
                                }
                            }
                            "connect" => {
                                if args.len() < 4 || args[2] != "--command" {
                                    println!(
                                        "Usage: /mcp connect <name> --command <cmd> [--args ...]"
                                    );
                                } else {
                                    let server_name = args[1].to_string();
                                    let cmd = args[3].to_string();
                                    let mut arg_list: Vec<String> = Vec::new();
                                    let mut i = 4;
                                    if i < args.len() && args[i] == "--args" {
                                        i += 1;
                                        while i < args.len() && !args[i].starts_with("--") {
                                            arg_list.push(args[i].to_string());
                                            i += 1;
                                        }
                                    }
                                    let config = McpServerConfig {
                                        name: server_name.clone(),
                                        command: Some(cmd),
                                        args: Some(arg_list),
                                        env: None,
                                        url: None,
                                        tool_name_prefix: None,
                                        auth: None,
                                    };
                                    match mcp_manager.connect_server(config).await {
                                        Ok(new_tools) => {
                                            let count = new_tools.len();
                                            if let Ok(mut locked) = all_tools.write() {
                                                locked.extend(new_tools);
                                            }
                                            println!("Connected to MCP server ({} tool(s))", count);
                                        }
                                        Err(e) => {
                                            println!("Failed to connect MCP server: {}", e);
                                        }
                                    }
                                }
                            }
                            "disconnect" => {
                                if args.len() < 3 {
                                    println!("Usage: /mcp disconnect <name>");
                                } else {
                                    let name = args[2];
                                    match mcp_manager.disconnect_server(name).await {
                                        Ok(removed_tools) => {
                                            if let Ok(mut locked) = all_tools.write() {
                                                for tool_name in &removed_tools {
                                                    locked.retain(|t| t.name != *tool_name);
                                                }
                                            }
                                            println!(
                                                "Disconnected MCP server '{}' ({} tool(s) removed)",
                                                name,
                                                removed_tools.len()
                                            );
                                        }
                                        Err(e) => {
                                            println!("Error: {}", e);
                                        }
                                    }
                                }
                            }
                            _ => {
                                println!("Unknown mcp subcommand: {}. Use /mcp for help.", args[1]);
                            }
                        }
                    }
                    continue;
                }

                if input == "/skill" || input.starts_with("/skill ") {
                    let sm = crate::core::settings::SettingsManager::load(&cwd);
                    if sm.get_enable_skill_commands() {
                        let skills = resource_loader.skills();
                        if skills.is_empty() {
                            println!("No skills loaded. Place SKILL.md files in:");
                            println!("  {}/skills/", crate::config::get_agent_dir().display());
                            println!("  .pick/skills/");
                        } else {
                            println!("Available skills ({}):", skills.len());
                            for skill in skills {
                                println!("  /skill:{} - {}", skill.name, skill.description);
                            }
                        }
                    } else {
                        println!("Skill commands are disabled. Enable them in settings.");
                    }
                    continue;
                }

                if input.starts_with("/skill:") {
                    let sm = crate::core::settings::SettingsManager::load(&cwd);
                    if !sm.get_enable_skill_commands() {
                        println!("Skill commands are disabled. Enable them in settings.");
                        continue;
                    }
                    let skill_name = input[7..]
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_string();
                    match crate::utils::frontmatter::expand_skill_command(
                        &input,
                        resource_loader.skills(),
                    ) {
                        Some(expanded) => {
                            if let Some(skill) = resource_loader
                                .skills()
                                .iter()
                                .find(|s| s.name == skill_name)
                            {
                                println!("{}", crate::modes::interactive::components::skill_invocation_message::render_skill_invocation(
                                    &skill.name, Some(&skill.description)
                                ));
                            }
                            input = expanded;
                        }
                        None => {
                            println!(
                                "Unknown skill: {}. Type /help to see available skills.",
                                skill_name
                            );
                            continue;
                        }
                    }
                }

                // Expand command templates from .pick/commands/ .md files
                if input.starts_with('/') {
                    let commands = resource_loader.commands();
                    let expanded =
                        crate::core::prompt_templates::expand_prompt_template(&input, commands);
                    if expanded != input {
                        input = expanded;
                    }
                }

                let mode_rules_for_hook = agent_mode.ruleset();

                if let Err(msg) = ensure_api_key(&auth, provider).await {
                    eprintln!("\x1b[31m{}\x1b[0m", msg);
                    all_messages.pop();
                    continue;
                }

                let prev_len = all_messages.len();

                all_messages.push(Message::User(UserMessage::text(&input)));

                println!();

                tools = pick_agent::tools::filter_goal_tools(
                    pick_agent::permission::disabled::filter_tools(
                        all_tools.read().unwrap().clone(),
                        &[&agent_mode.ruleset()],
                    ),
                    session_manager.goal_manager(),
                );

                permission_manager.register_permission_hook(std::sync::Arc::new(
                    pick_agent::permission::hooks::CliApprovalHook::new(),
                ));

                // Track how much of the assistant text has been printed to stdout,
                // so MessageUpdate events only print the delta (not the full text).
                let printed_offset = std::sync::atomic::AtomicUsize::new(0);

                let config = AgentLoopConfig {
                    model: model.clone(),
                    system_prompt: system_prompt.clone(),
                    tools: tools.clone(),
                    thinking_level,
                    max_tokens: None,
                    temperature: None,
                    extension_runner: extension_runner.clone(),
                    transform_context: None,
                    get_api_key: None,
                    fs_policy: permission_manager.fs_policy(),
                    cwd: Some(std::env::current_dir().unwrap_or_default()),
                    permission_hooks: Some(permission_manager.hook_registry.clone()),
                    mode_rulesets: Some(vec![mode_rules_for_hook.clone()]),
                    permission_manager: Some(permission_manager.clone()),
                    sandbox: platform_sandbox.clone(),
                    sandbox_enabled: Some(sandbox_enabled.clone()),
                    cancel_signal_tx: None,
                    before_tool_call: Some(Arc::new({
                        let mode_rules = mode_rules_for_hook.clone();
                        move |tc: &pick_ai::types::ToolCall| -> Option<String> {
                            let tool_args_str = if let Some(cmd) =
                                tc.arguments.get("command").and_then(|c| c.as_str())
                            {
                                cmd.to_string()
                            } else if let Some(path) =
                                tc.arguments.get("path").and_then(|p| p.as_str())
                            {
                                path.to_string()
                            } else {
                                tc.arguments.to_string()
                            };

                            pick_agent::permission::evaluate::check_permission(
                                &tc.name,
                                &tool_args_str,
                                &[&mode_rules],
                            )
                            .err()
                        }
                    })),
                    should_stop_after_turn: None,
                    get_steering_messages: Some(Arc::new({
                        let mode_shared = mode_shared.clone();
                        let goal_manager = session_manager.goal_manager();
                        move || {
                            let mode = *mode_shared.lock().unwrap();
                            let mut msgs: Vec<Message> = match mode {
                                AgentMode::Plan => {
                                    vec![Message::User(UserMessage::text(PLAN_MODE_REMINDER))]
                                }
                                AgentMode::Build => vec![],
                            };
                            if let Some(goal) = goal_manager.get()
                                && goal.status == "active"
                            {
                                let objective =
                                    crate::modes::tui::agent_exec::escape_xml_text(&goal.objective);
                                let token_budget_str = goal
                                    .token_budget
                                    .map(|b| b.to_string())
                                    .unwrap_or_else(|| "none".to_string());
                                let remaining_tokens = goal_manager
                                    .remaining_tokens()
                                    .map(|r| r.to_string())
                                    .unwrap_or_else(|| "unbounded".to_string());
                                let msg_text = crate::modes::tui::agent_exec::render_goal_template(
                                    include_str!("../../templates/goals/steering_active.md"),
                                    &[
                                        ("objective", &objective),
                                        ("tokens_used", &goal.tokens_used.to_string()),
                                        ("token_budget", &token_budget_str),
                                        ("remaining_tokens", &remaining_tokens),
                                        ("time_used_seconds", &goal.time_used_seconds.to_string()),
                                    ],
                                );
                                msgs.push(Message::User(UserMessage::text(msg_text)));
                            }
                            msgs
                        }
                    })),
                    get_follow_up_messages: None,
                    provider_max_retries: None,
                    provider_max_retry_delay_ms: None,
                    approve: None,
                    question: Some(std::sync::Arc::new(
                        move |questions: Vec<QuestionPrompt>| {
                            Box::pin(async move {
                                let mut answers = Vec::new();
                                for (i, q) in questions.iter().enumerate() {
                                    print!(
                                        "\n\x1b[1m[{}/{}] {}\x1b[0m\n",
                                        i + 1,
                                        questions.len(),
                                        q.question
                                    );
                                    for (j, opt) in q.options.iter().enumerate() {
                                        println!(
                                            "  {}. {} — {}",
                                            j + 1,
                                            opt.label,
                                            opt.description
                                        );
                                    }
                                    print!("  > ");
                                    std::io::stdout().flush().ok();
                                    let mut input = String::new();
                                    std::io::stdin().read_line(&mut input).ok();
                                    let trimmed = input.trim().to_string();
                                    if let Ok(num) = trimmed.parse::<usize>()
                                        && num >= 1
                                        && num <= q.options.len()
                                    {
                                        answers.push(vec![q.options[num - 1].label.clone()]);
                                        continue;
                                    }
                                    answers.push(vec![trimmed]);
                                }
                                Ok(answers)
                            })
                        },
                    )),
                    agent_id: None,
                    agent_registry: Some(agent_registry.clone()),
                    on_turn_complete: None,
                    on_event: Some(Arc::new(move |event| {
                        match event {
                            AgentEvent::TurnStart => {
                                print!("\x1b[1mAssistant:\x1b[0m ");
                                printed_offset.store(0, std::sync::atomic::Ordering::Relaxed);
                            }
                            AgentEvent::MessageStart { message } => {
                                if let Message::Assistant(msg) = message {
                                    let mut offset =
                                        printed_offset.load(std::sync::atomic::Ordering::Relaxed);
                                    for block in &msg.content {
                                        if let ContentBlock::Text(t) = block {
                                            let text = &t.text;
                                            if offset < text.len() {
                                                print!("{}", &text[offset..]);
                                                offset = text.len();
                                            }
                                        }
                                    }
                                    printed_offset
                                        .store(offset, std::sync::atomic::Ordering::Relaxed);
                                }
                            }
                            AgentEvent::MessageUpdate { message, .. } => {
                                if let Message::Assistant(msg) = message {
                                    let mut offset =
                                        printed_offset.load(std::sync::atomic::Ordering::Relaxed);
                                    for block in &msg.content {
                                        if let ContentBlock::Text(t) = block {
                                            let text = &t.text;
                                            if offset < text.len() {
                                                print!("{}", &text[offset..]);
                                                offset = text.len();
                                            }
                                        }
                                    }
                                    printed_offset
                                        .store(offset, std::sync::atomic::Ordering::Relaxed);
                                }
                            }
                            AgentEvent::ToolExecutionStart {
                                ref tool_name,
                                ref args,
                                ..
                            } => {
                                let render_ctx = ToolRenderContext {
                                    args: Some(args.clone()),
                                    cwd: String::new(),
                                    expanded: false,
                                    show_images: false,
                                    is_error: false,
                                };
                                if let Some(output) = render_tool_call(tool_name, args, &render_ctx)
                                {
                                    if !output.label.is_empty() {
                                        print!("\n{}", output.label);
                                    } else {
                                        print!(
                                            "\n{} {} ",
                                            ToolTheme::fg("toolTitle", tool_name),
                                            ToolTheme::fg("dim", "...")
                                        );
                                    }
                                } else {
                                    print!(
                                        "\n{} {} ",
                                        ToolTheme::fg("toolTitle", tool_name),
                                        ToolTheme::fg("dim", "...")
                                    );
                                }
                            }
                            AgentEvent::ToolExecutionEnd {
                                result, is_error, ..
                            } => {
                                if is_error {
                                    let error_text = result
                                        .get("error")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("Unknown error");
                                    print!(
                                        " {}",
                                        ToolTheme::fg("error", &format!("[Error: {}]", error_text))
                                    );
                                } else if let Some(content) = result.get("content") {
                                    if let Some(texts) = content.as_array() {
                                        let mut has_output = false;
                                        for t in texts {
                                            if let Some(text) = t.as_str()
                                                && !text.is_empty()
                                            {
                                                if !has_output {
                                                    println!();
                                                    has_output = true;
                                                }
                                                print!("{}", ToolTheme::fg("toolOutput", text));
                                            }
                                        }
                                    }
                                } else {
                                    print!(" {}", ToolTheme::fg("success", "[done]"));
                                }
                            }
                            AgentEvent::TurnEnd { .. } => {
                                println!();
                            }
                            _ => {}
                        }
                        use std::io::Write;
                        std::io::stdout().flush().ok();
                    })),
                };

                match crate::core::agent_session::run_agent_loop_with_retry(
                    config,
                    all_messages.clone(),
                    Default::default(),
                    None,
                )
                .await
                {
                    Ok(result) => {
                        for msg in &result.messages[prev_len..] {
                            if let Err(e) = session_manager.append(SessionEntry::from(msg)).await {
                                eprintln!("\x1b[31mWarning: session persist failed: {}\x1b[0m", e);
                            }
                        }

                        let last_msg = result.messages.last();
                        let has_tool_calls = last_msg.is_some_and(|m| {
                            if let Message::Assistant(msg) = m {
                                msg.content
                                    .iter()
                                    .any(|c| matches!(c, ContentBlock::ToolCall(_)))
                            } else {
                                false
                            }
                        });

                        if !has_tool_calls {
                            all_messages = result.messages;
                        } else {
                            all_messages = result.messages;
                        }

                        let usage = &result.usage;
                        println!(
                            "\x1b[2m[Input: {} | Output: {} | Cache: {}/{}]\x1b[0m",
                            usage.input, usage.output, usage.cache_read, usage.cache_write
                        );

                        let compact_settings = CompactionSettings::default();
                        if should_compact(
                            usage.total_tokens as usize,
                            model.context_window as usize,
                            &compact_settings,
                        ) {
                            println!(
                                "\x1b[2mAuto-compacting ({} tokens / {} window)...\x1b[0m",
                                usage.total_tokens, model.context_window
                            );
                            let path_entries: Vec<serde_json::Value> = all_messages.iter().map(|msg| {
                                let id = uuid::Uuid::now_v7().to_string();
                                let message_val = match msg {
                                    Message::User(u) => serde_json::json!({
                                        "role": "user",
                                        "content": u.content,
                                    }),
                                    Message::Assistant(a) => serde_json::json!({
                                        "role": "assistant",
                                        "content": a.content,
                                        "stopReason": format!("{:?}", a.stop_reason),
                                    }),
                                    Message::ToolResult(t) => serde_json::json!({
                                        "role": "toolResult",
                                        "content": t.content,
                                        "toolCallId": t.tool_call_id,
                                        "toolName": t.tool_name,
                                        "isError": t.is_error,
                                    }),
                                };
                                serde_json::json!({"id": id, "type": "message", "message": message_val})
                            }).collect();

                            if let Some(ref runner) = extension_runner {
                                runner.emit(&ExtensionEvent::SessionBeforeCompact(
                                    SessionBeforeCompactEvent {
                                        preparation: serde_json::json!({}),
                                        branch_entries: path_entries.clone(),
                                        custom_instructions: None,
                                    },
                                ));
                            }

                            let api_key =
                                auth.get_api_key(provider, true).await.unwrap_or_default();
                            if let Some(preparation) =
                                prepare_compaction(&path_entries, &compact_settings)
                            {
                                match compact(&preparation, &model, &api_key, None, None, None)
                                    .await
                                {
                                    Ok(result) => {
                                        let summary = result.summary;
                                        let before = all_messages.len();
                                        all_messages =
                                            vec![Message::User(UserMessage::text(format!(
                                                "[Compacted conversation summary]\n\n{}",
                                                summary
                                            )))];
                                        println!(
                                            "\x1b[2mAuto-compacted ({} msgs → 1, {} tokens before).\x1b[0m",
                                            before, result.tokens_before
                                        );
                                        let compact_entry = SessionEntry {
                                            id: uuid::Uuid::now_v7().to_string(),
                                            parent_id: None,
                                            timestamp: chrono::Utc::now().timestamp_millis(),
                                            kind: SessionEntryKind::Compaction(CompactionEntry {
                                                summary: summary.clone(),
                                                token_count: Some(result.tokens_before as u64),
                                            }),
                                        };
                                        if let Err(e) = session_manager.append(compact_entry).await
                                        {
                                            eprintln!(
                                                "Warning: failed to persist compaction entry: {}",
                                                e
                                            );
                                        }
                                        if let Some(ref runner) = extension_runner {
                                            runner.emit(&ExtensionEvent::SessionCompact(
                                                SessionCompactEvent {
                                                    compaction_entry: serde_json::json!({
                                                        "summary": summary,
                                                        "tokensBefore": result.tokens_before,
                                                    }),
                                                    from_extension: false,
                                                },
                                            ));
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "\x1b[31mAuto-compaction failed: {}\x1b[0m",
                                            e.message
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("\x1b[31mError: {}\x1b[0m", e);
                    }
                }

                println!();
            }
        }
    }
}

async fn switch_agent_mode(
    mode: &mut AgentMode,
    new_mode: AgentMode,
    session_manager: &mut SessionManager,
) {
    if *mode == new_mode {
        return;
    }
    let old = *mode;
    *mode = new_mode;
    let change_entry = SessionEntry {
        id: uuid::Uuid::now_v7().to_string(),
        parent_id: None,
        timestamp: chrono::Utc::now().timestamp_millis(),
        kind: SessionEntryKind::AgentModeChange(pick_agent::session::AgentModeChangeEntry {
            from: old.to_string(),
            to: new_mode.to_string(),
        }),
    };
    if let Err(e) = session_manager.append(change_entry).await {
        eprintln!("Warning: failed to persist mode change: {}", e);
    }
}

async fn ensure_api_key(auth: &AuthStorage, provider: &str) -> Result<(), String> {
    let env_var = format!("{}_API_KEY", provider.to_uppercase().replace('-', "_"));

    if std::env::var(&env_var).is_ok() {
        return Ok(());
    }

    if let Some(key) = auth.get_api_key(provider, true).await {
        unsafe {
            std::env::set_var(&env_var, key);
        }
        return Ok(());
    }

    eprint!("Enter API key for {}: ", provider);
    use std::io::Write;
    std::io::stderr()
        .flush()
        .map_err(|_| "Failed to flush stderr".to_string())?;
    let mut key = String::new();
    std::io::stdin()
        .read_line(&mut key)
        .map_err(|_| "Failed to read input".to_string())?;
    let key = key.trim().to_string();

    if key.is_empty() {
        Err(format!(
            "No API key available for '{}'. Set {} or use --api-key.",
            provider, env_var
        ))
    } else {
        unsafe {
            std::env::set_var(&env_var, &key);
        }
        Ok(())
    }
}
