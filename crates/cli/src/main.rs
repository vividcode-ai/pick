//! Pick CLI - Main entry point

// Allow dead code - many module items are pub for cross-module use but reported as
// dead in a binary crate. Remove this if refactoring to a lib+bin split.
#![allow(dead_code)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::never_loop)]
#![allow(clippy::manual_strip)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::unnecessary_sort_by)]
#![allow(clippy::vec_init_then_push)]
#![allow(clippy::enum_variant_names)]
#![allow(clippy::format_in_format_args)]
#![allow(clippy::module_inception)]
#![allow(clippy::manual_flatten)]
#![allow(clippy::await_holding_lock)]
#![allow(clippy::collapsible_if)]

mod args;
mod cli;
mod config;
mod core;
mod modes;
mod notification;
mod utils;

use pick_agent::permission::sandbox::Sandbox as _;

use args::{parse_args, print_help};
use config::VERSION;
use core::auth_storage::AuthStorage;
use core::migrations::run_migrations;
use core::session::create_session_manager;
use core::settings::{Settings, SettingsManager};
use core::update_action::UpdateAction;
#[cfg_attr(debug_assertions, allow(unused_imports))]
use core::update_action::get_update_action;
use modes::{run_audit_command, run_interactive_mode, run_print_mode, run_rpc_mode, run_tui_mode};

use pick_agent::agent_registry::AgentRegistry;
use pick_agent::extensions::discover_and_load_extensions;
use pick_agent::extensions::runner::ExtensionRunner;
use pick_agent::permission::manager::PermissionManager;
use pick_agent::tools::{create_coding_tools_with_goal_manager, create_extension_tools};
use pick_ai::providers::register::register_builtins;
use pick_ai::types::{Message, UserMessage};
use pick_mcp::McpManager;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

async fn run_update_command() -> anyhow::Result<()> {
    #[cfg(debug_assertions)]
    {
        anyhow::bail!(
            "`pick update` is not available in debug builds. Install a release build to use this command."
        );
    }

    #[cfg(not(debug_assertions))]
    {
        use crate::core::install_context::{InstallContext, InstallMethod};
        use crate::core::updates::fetch_latest_version;
        use crate::utils::version_check::is_newer_package_version;

        let ctx = InstallContext::current();

        // Print which source we're checking
        match ctx.method {
            InstallMethod::Npm => {
                println!(
                    "Checking npm registry (registry.npmjs.org/@vividcodeai/pick) for updates..."
                );
            }
            InstallMethod::GitHub { .. } | InstallMethod::Cargo => {
                println!(
                    "Checking GitHub releases (api.github.com/repos/vividcode-ai/pick) for updates..."
                );
            }
            InstallMethod::Other => {
                println!("Source build detected \u{2014} unable to auto-update.");
                println!(
                    "Please update manually: https://github.com/vividcode-ai/pick/releases/latest"
                );
                return Ok(());
            }
        }

        match fetch_latest_version(ctx).await {
            Some(latest) if is_newer_package_version(&latest, VERSION) => {
                println!("New version available: v{latest} (current: v{VERSION})");
                println!("Downloading...");
            }
            _ => {
                println!("Pick v{VERSION} is already up to date.");
                return Ok(());
            }
        }

        let Some(action) = get_update_action() else {
            anyhow::bail!(
                "Could not detect the Pick installation method. Please update manually: https://github.com/vividcode-ai/pick/releases/latest"
            );
        };
        run_update_action(action)
    }
}

fn run_update_action(action: UpdateAction) -> anyhow::Result<()> {
    if matches!(action, UpdateAction::Manual) {
        anyhow::bail!(
            "Manual update required. Download from: https://github.com/vividcode-ai/pick/releases/latest"
        );
    }

    let cmd_str = action.command_str();
    println!();
    println!("Updating Pick via `{}` ...", cmd_str);

    let status = {
        #[cfg(windows)]
        {
            if matches!(action, UpdateAction::GitHubStandaloneWindows) {
                let (cmd, args) = action.command_args();
                std::process::Command::new(cmd).args(args).status()?
            } else {
                std::process::Command::new("cmd")
                    .args(["/C", &cmd_str])
                    .status()?
            }
        }
        #[cfg(not(windows))]
        {
            let (cmd, args) = action.command_args();
            std::process::Command::new(cmd).args(args).status()?
        }
    };

    if !status.success() {
        anyhow::bail!("`{}` failed with status {}", cmd_str, status);
    }

    println!();
    println!("Update ran successfully! Please restart Pick.");
    Ok(())
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("info,rmcp=warn,pick_mcp=warn")
            }),
        )
        .init();

    // Register built-in AI providers
    register_builtins();

    // Parse CLI arguments
    let args = parse_args(std::env::args().skip(1).collect());

    if args.help {
        print_help();
        return;
    }

    if args.version {
        println!("{} v{}", crate::config::APP_NAME, VERSION);
        return;
    }

    // Handle update subcommand
    if args.update {
        if let Err(e) = run_update_command().await {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // Run migrations (pass cwd for project-local migrations)
    let cwd = std::env::current_dir().unwrap_or_default();
    let migration_result = run_migrations(&cwd);
    if !migration_result.deprecation_warnings.is_empty() {
        for warning in &migration_result.deprecation_warnings {
            tracing::warn!("{}", warning);
        }
    }

    // Workspace trust check: verify the current directory is trusted before
    // proceeding. Skip for non-interactive modes (print/json/rpc).
    let is_interactive =
        (args.mode.is_empty() && !args.print) || args.mode == "tui" || args.mode == "interactive";
    if is_interactive
        && !core::workspace_trust::is_workspace_trusted(&cwd)
        && !core::workspace_trust::confirm_and_trust_workspace(&cwd)
    {
        std::process::exit(0);
    }

    // Load settings for default provider/model
    let mut settings = SettingsManager::load(&cwd);

    // Initialize settings files if needed and log all configured settings
    if let Err(e) = settings.set_global(Settings::default()) {
        tracing::warn!("Failed to initialize global settings file: {}", e);
    }
    if let Err(e) = settings.set_project(Settings::default()) {
        tracing::warn!("Failed to initialize project settings file: {}", e);
    }
    settings.reload(&cwd);
    tracing::debug!(
        "Settings: provider={:?} model={:?} thinking={:?} transport={:?} theme={:?} shell={:?} session_dir={:?}",
        settings.default_provider(),
        settings.default_model(),
        settings.default_thinking_level(),
        settings.transport(),
        settings.theme(),
        settings.shell_path(),
        settings.session_dir(),
    );
    let args = {
        let mut args = args;
        if args.provider.is_none() {
            args.provider = settings.default_provider().map(String::from);
        }
        if args.model.is_none() {
            args.model = settings.default_model().map(String::from);
        }
        if args.thinking.is_none() {
            args.thinking = settings.default_thinking_level().map(String::from);
        }
        // --print flag sets mode to "print" (unless mode was explicitly set)
        if args.print && args.mode.is_empty() {
            args.mode = "print".to_string();
        }
        args
    };

    // Handle --list-models: print models and exit
    if args.list_models.is_some() {
        let providers = pick_ai::models::get_providers();
        for p in &providers {
            let models = pick_ai::models::get_models(p);
            for m in &models {
                println!("{}  ({})  [{}]", m.id, m.name, m.provider.as_str());
            }
        }
        return;
    }

    // Handle --export: export session to HTML and exit
    if let Some(ref export_path) = args.export_html {
        if let Some(ref session_id) = args.session {
            use crate::core::export_html::export_html::{ExportOptions, export_from_data};
            // Resolve session file path
            let session_file = {
                let project_path = cwd
                    .join(".pick")
                    .join("sessions")
                    .join(format!("{}.jsonl", session_id));
                if project_path.exists() {
                    project_path
                } else {
                    crate::config::get_sessions_dir().join(format!("{}.jsonl", session_id))
                }
            };
            if !session_file.exists() {
                eprintln!("Error: Session '{}' not found.", session_id);
                return;
            }
            match std::fs::read_to_string(&session_file) {
                Ok(content) => {
                    let lines: Vec<&str> =
                        content.lines().filter(|l| !l.trim().is_empty()).collect();
                    let header: serde_json::Value = lines
                        .first()
                        .and_then(|l| serde_json::from_str(l).ok())
                        .unwrap_or(serde_json::json!({}));
                    let header = ensure_timestamp_field(header);
                    let entries: Vec<serde_json::Value> = lines[1..]
                        .iter()
                        .filter_map(|l| serde_json::from_str(l).ok())
                        .map(restructure_entry)
                        .collect();
                    let options = ExportOptions {
                        output_path: Some(export_path.clone()),
                        theme_name: None,
                    };
                    match export_from_data(entries, header, None, None, None, Some(&options)) {
                        Ok(path) => println!("Exported session to: {}", path),
                        Err(e) => eprintln!("Export failed: {}", e),
                    }
                }
                Err(e) => eprintln!("Error reading session file: {}", e),
            }
        } else {
            eprintln!("Error: --export requires --session <ID>");
        }
        return;
    }

    // Handle --audit: view permission audit trail and exit (early, no need for full init)
    if args.audit {
        run_audit_command(&args, &cwd).await;
        return;
    }

    // Initialize auth storage
    let auth = Arc::new(AuthStorage::create(None));

    // Log configured providers
    let providers = auth.list_providers();
    if !providers.is_empty() {
        tracing::debug!("Stored credentials for providers: {:?}", providers);
    }

    // Set runtime API key from --api-key flag
    if let Some(ref api_key) = args.api_key
        && let Some(ref provider) = args.provider
    {
        auth.set_runtime_api_key(provider, api_key.clone());
    }

    // Discover and load extensions (skipped if --no-extensions)
    let load_result = if args.no_extensions {
        pick_agent::extensions::types::LoadExtensionsResult {
            extensions: Vec::new(),
            errors: Vec::new(),
        }
    } else {
        let ext_paths: Vec<String> = {
            let mut paths = args.extensions.clone();
            if let Some(ref ext_settings) = settings.get().extensions {
                for p in ext_settings {
                    if !paths.contains(p) {
                        paths.push(p.clone());
                    }
                }
            }
            paths
        };
        discover_and_load_extensions(&ext_paths, &cwd, &cwd).await
    };

    // Report extension load errors (non-fatal)
    for err in &load_result.errors {
        eprintln!(
            "Warning: failed to load extension '{}': {}",
            err.path, err.error
        );
    }

    // Create extension runner for registered extensions
    let extension_runner = if load_result.extensions.is_empty() {
        None
    } else {
        Some(std::sync::Arc::new(ExtensionRunner::new(
            load_result.extensions.clone(),
        )))
    };

    // Determine agent mode (default: build)
    use crate::core::agent_mode::AgentMode;
    let agent_mode = args
        .agent_mode
        .as_deref()
        .and_then(|m| m.parse::<AgentMode>().ok())
        .unwrap_or(AgentMode::Build);

    // Create MCP manager
    let mcp_manager = Arc::new(McpManager::new());

    // Parse MCP server configs (fast, no I/O)
    let mut mcp_configs: Vec<pick_mcp::McpServerConfig> = settings
        .get()
        .mcp_servers
        .clone()
        .map(|servers| pick_mcp::parse_mcp_configs_from_value(&serde_json::json!(servers)))
        .unwrap_or_default();

    // Filter out disabled MCP servers (saved in settings)
    let disabled_servers = settings.get_disabled_mcp_servers();
    if !disabled_servers.is_empty() {
        mcp_configs.retain(|c| !disabled_servers.contains(&c.name));
    }

    // Create session manager (before tools so GoalManager is available)
    let session_dir = settings.session_dir().map(|p| p.to_path_buf());
    let session_manager = match create_session_manager(&args, &cwd, session_dir).await {
        Ok(mgr) => mgr,
        Err(e) => {
            if e == "cancelled" {
                std::process::exit(0);
            }
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // Create tools (built-in + extension) — without MCP tools (loaded in background)
    let tools = {
        let mut tools = if args.no_builtin_tools || args.no_tools {
            Vec::new()
        } else {
            create_coding_tools_with_goal_manager(
                Some(agent_mode.to_string()),
                session_manager.goal_manager(),
            )
        };
        if let Some(ref runner) = extension_runner {
            let ext_tools = create_extension_tools(runner.clone());
            if !ext_tools.is_empty() {
                tracing::info!("Loaded {} extension tool(s)", ext_tools.len());
            }
            tools.extend(ext_tools);
        }
        // Apply --tools allowlist filter
        if !args.tools.is_empty() {
            tools.retain(|t| args.tools.contains(&t.name));
        }
        tools
    };

    // Shared tool list (allows runtime modification for dynamic MCP connect/disconnect)
    let all_tools: Arc<RwLock<Vec<pick_agent::core::state::AgentTool>>> =
        Arc::new(RwLock::new(tools));

    // Background MCP connection: spawn task and pass notification channel to mode
    let (mcp_done_tx, mcp_done_rx) = tokio::sync::watch::channel(false);
    let mcp_cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));

    if !mcp_configs.is_empty() {
        let bg_mgr = mcp_manager.clone();
        let bg_tools = all_tools.clone();
        let bg_configs = mcp_configs;
        let bg_done = mcp_done_tx;
        let bg_cancel = mcp_cancelled.clone();

        tokio::spawn(async move {
            let tools = bg_mgr.connect_from_config(&bg_configs).await;
            if !bg_cancel.load(std::sync::atomic::Ordering::Relaxed) {
                let count = tools.len();
                if count > 0
                    && let Ok(mut locked) = bg_tools.write()
                {
                    locked.extend(tools);
                }
                bg_done.send(true).ok();
            }
        });
    }

    // Build initial messages
    let mut initial_messages: Vec<Message> = Vec::new();
    if !args.messages.is_empty() {
        for msg in &args.messages {
            initial_messages.push(Message::User(UserMessage::text(msg)));
        }
    }

    // Build permission manager from settings
    let permission_config = settings.get_permission();
    let profile_str = &permission_config.permission_profile;
    let global_rules_path = crate::config::get_agent_dir()
        .join("rules")
        .join("default.rules");
    let project_rules_path = cwd.join(".pick").join("rules").join("default.rules");
    let mut rules_files: Vec<String> = Vec::new();
    if global_rules_path.exists() {
        rules_files.push(global_rules_path.to_string_lossy().to_string());
    }
    if project_rules_path.exists() {
        rules_files.push(project_rules_path.to_string_lossy().to_string());
    }
    let permission_manager =
        PermissionManager::new(profile_str, &cwd, Some(&permission_config), &rules_files);

    // Initialize theme system with the configured theme name and file watcher
    crate::core::theme::init_theme(settings.theme(), true);

    // Create the agent registry for in-process subagent spawning
    let agent_registry = AgentRegistry::new();

    // Take a snapshot of the current tool list for print/rpc modes (snapshot at startup)
    let tools_snapshot = all_tools.read().map_err(|e| e.to_string()).unwrap().clone();

    // Initialize platform sandbox
    let default_sandbox_config = pick_agent::permission::sandbox::SandboxConfig::default();
    let sandbox_config = permission_manager
        .sandbox_config
        .as_ref()
        .unwrap_or(&default_sandbox_config);

    // Try to create and activate a platform sandbox
    let platform_sandbox: Option<std::sync::Arc<dyn pick_agent::permission::sandbox::Sandbox>> = {
        #[cfg(target_os = "windows")]
        {
            let s = pick_sandbox::platforms::windows_restricted_token::WindowsRestrictedTokenSandbox::new(sandbox_config);
            if s.is_available() {
                Some(std::sync::Arc::new(s))
            } else {
                None
            }
        }
        #[cfg(target_os = "linux")]
        {
            let s = pick_sandbox::platforms::linux_bwrap::LinuxBwrapSandbox::new(sandbox_config);
            if s.is_available() {
                Some(std::sync::Arc::new(s))
            } else {
                None
            }
        }
        #[cfg(target_os = "macos")]
        {
            let s =
                pick_sandbox::platforms::macos_seatbelt::MacosSeatbeltSandbox::new(sandbox_config);
            if s.is_available() {
                Some(std::sync::Arc::new(s))
            } else {
                None
            }
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            None
        }
    };

    // Determine mode and run
    let pm = Arc::new(permission_manager);
    let platform_sandbox = platform_sandbox; // shadow for clarity
    let sandbox_enabled = Arc::new(AtomicBool::new(settings.get_permission().sandbox_enabled));
    let update_action = match args.mode.as_str() {
        "rpc" => {
            run_rpc_mode(
                args,
                tools_snapshot,
                auth,
                session_manager,
                extension_runner,
                agent_mode,
                agent_registry,
                pm,
                platform_sandbox,
                sandbox_enabled,
            )
            .await;
            None
        }
        "json" | "print" => {
            run_print_mode(
                args,
                tools_snapshot,
                auth,
                session_manager,
                initial_messages,
                extension_runner,
                agent_mode,
                agent_registry,
                pm,
                platform_sandbox,
                sandbox_enabled,
            )
            .await;
            None
        }
        "tui" => {
            run_tui_mode(
                args,
                all_tools,
                auth,
                session_manager,
                initial_messages,
                extension_runner,
                agent_mode,
                agent_registry,
                mcp_manager,
                mcp_done_rx,
                mcp_cancelled,
                pm,
                platform_sandbox,
                sandbox_enabled.clone(),
            )
            .await
        }
        "interactive" => {
            run_interactive_mode(
                args,
                all_tools,
                auth,
                session_manager,
                initial_messages,
                extension_runner,
                agent_mode,
                agent_registry,
                mcp_manager,
                mcp_done_rx,
                mcp_cancelled,
                pm,
                platform_sandbox,
                sandbox_enabled.clone(),
            )
            .await;
            None
        }
        _ => {
            run_tui_mode(
                args,
                all_tools,
                auth,
                session_manager,
                initial_messages,
                extension_runner,
                agent_mode,
                agent_registry,
                mcp_manager,
                mcp_done_rx,
                mcp_cancelled,
                pm,
                platform_sandbox,
                sandbox_enabled.clone(),
            )
            .await
        }
    };

    // Execute pending update action after TUI exits
    if let Some(action) = update_action
        && let Err(e) = run_update_action(action)
    {
        eprintln!("Error updating Pick: {}", e);
    }
}

/// Ensure the header JSON has a `timestamp` field for the HTML export template.
/// The template.js renders Date from `header.timestamp`, but the JSONL file
/// stores timestamps as `created_at` (serde snake_case default).
fn ensure_timestamp_field(mut header: serde_json::Value) -> serde_json::Value {
    if let Some(obj) = header.as_object_mut() {
        if !obj.contains_key("timestamp") {
            if let Some(created_at) = obj.get("created_at").or_else(|| obj.get("createdAt")) {
                obj.insert("timestamp".to_string(), created_at.clone());
            }
        }
    }
    header
}

/// Restructure a raw JSONL entry (serde flattened format) into the nested
/// format expected by the HTML export JS template.
///
/// The session file stores entries with `#[serde(flatten)]`, so message
/// fields like `role`, `content`, `stop_reason` sit at the top level
/// alongside `id`, `type`, `parent_id`, `timestamp`. The JS template
/// expects message fields nested under `entry.message` with camelCase keys.
fn restructure_entry(mut entry: serde_json::Value) -> serde_json::Value {
    const SNAKE_TO_CAMEL_KEYS: &[(&str, &str)] = &[
        ("parent_id", "parentId"),
        ("stop_reason", "stopReason"),
        ("tool_call_id", "toolCallId"),
        ("tool_name", "toolName"),
        ("is_error", "isError"),
        ("error_message", "errorMessage"),
        ("exit_code", "exitCode"),
        ("token_count", "tokensBefore"),
        ("thinking_signature", "thinkingSignature"),
        ("response_id", "responseId"),
    ];

    let Some(obj) = entry.as_object_mut() else {
        return entry;
    };

    // Rename snake_case fields to camelCase at the top level
    for &(snake, camel) in SNAKE_TO_CAMEL_KEYS {
        if let Some(val) = obj.remove(snake) {
            obj.insert(camel.to_string(), val);
        }
    }

    // For message-type entries, nest message-related fields under "message"
    if obj.get("type").and_then(|v| v.as_str()) == Some("message") {
        let mut message_obj = serde_json::Map::new();

        for key in ["role", "content", "api", "provider", "model"] {
            if let Some(val) = obj.remove(key) {
                message_obj.insert(key.to_string(), val);
            }
        }

        // camelCase fields that belong inside message
        for key in [
            "stopReason",
            "toolCallId",
            "toolName",
            "isError",
            "errorMessage",
            "exitCode",
            "usage",
            "timestamp",
            "responseId",
        ] {
            if let Some(val) = obj.remove(key) {
                message_obj.insert(key.to_string(), val);
            }
        }

        obj.insert(
            "message".to_string(),
            serde_json::Value::Object(message_obj),
        );
    }

    // Map model_change fields: from/to → provider/modelId
    if obj.get("type").and_then(|v| v.as_str()) == Some("model_change") {
        if let Some(to_val) = obj.remove("to") {
            obj.insert("modelId".to_string(), to_val);
        }
        if !obj.contains_key("provider") {
            obj.insert(
                "provider".to_string(),
                serde_json::Value::String(String::new()),
            );
        }
    }

    // Map thinking_level_change: to → thinkingLevel
    if obj.get("type").and_then(|v| v.as_str()) == Some("thinking_level_change") {
        if let Some(to_val) = obj.remove("to") {
            obj.insert("thinkingLevel".to_string(), to_val);
        }
    }

    entry
}
