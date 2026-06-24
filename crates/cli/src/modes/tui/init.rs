use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use tokio::sync::mpsc;

use crate::args::Args;
use crate::core::agent_mode::AgentMode;
use crate::core::auth_storage::AuthStorage;
use crate::core::resource_loader::ResourceLoader;
use crate::core::settings::SettingsManager;
use pick_agent::core::message_queue::{PendingMessageQueue, QueueMode};
use pick_agent::core::state::{AgentTool, ThinkingLevel};
use pick_agent::extensions::runner::ExtensionRunner;
use pick_agent::session::SessionManager;
use pick_ai::models::get_model;
use pick_ai::types::Message;
use pick_ai::types::Model as AiModel;
use pick_mcp::McpManager;
use pick_tui::app::TuiApp;
use pick_tui::autocomplete::CombinedAutocompleteProvider;
use pick_tui::autocomplete::SlashCommand as TuiSlashCommand;
use pick_tui::terminal_manager::TerminalManager;

use super::context::TuiContext;
use super::event_handler;
use super::message_utils;
use super::types::*;

/// Initialize all TUI mode state.
/// Returns (ctx, cmd_rx, evt_rx, git_timer) where ctx owns all shareable state.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn init_tui_mode(
    args: Args,
    all_tools: Arc<RwLock<Vec<AgentTool>>>,
    auth: Arc<AuthStorage>,
    session_manager: SessionManager,
    initial_messages: Vec<Message>,
    extension_runner: Option<Arc<ExtensionRunner>>,
    agent_mode: AgentMode,
    agent_registry: Arc<pick_agent::agent_registry::AgentRegistry>,
    mcp_manager: Arc<McpManager>,
    _mcp_done_rx: tokio::sync::watch::Receiver<bool>,
    mcp_cancelled: Arc<AtomicBool>,
    permission_manager: Arc<pick_agent::permission::manager::PermissionManager>,
    platform_sandbox: Option<Arc<dyn pick_agent::permission::sandbox::Sandbox>>,
) -> (
    TuiContext,
    mpsc::UnboundedReceiver<TuiCommand>,
    mpsc::UnboundedReceiver<crossterm::event::Event>,
) {
    let version = crate::config::VERSION;
    let app_name = crate::config::APP_NAME;
    let cwd = std::env::current_dir().unwrap_or_default();

    // Filter tools based on agent mode
    let tools = filter_tools_by_mode(&all_tools, &agent_mode, &session_manager);

    // Resolve model + provider
    let (model, provider, model_id) = resolve_model(&args);

    // Set API key from auth storage
    update_api_key(&auth, &provider).await;

    // Resolve thinking level
    let thinking_level = resolve_thinking_level(args.thinking.as_deref(), &model);

    let tools = if args.no_tools { vec![] } else { tools };

    // Load resources
    let agent_dir = crate::config::get_agent_dir();
    let mut resource_loader = ResourceLoader::new(cwd.clone(), agent_dir);
    resource_loader.reload(&args.extensions).await;

    // Build system prompt
    let system_prompt = build_prompt(
        &tools,
        &resource_loader,
        &cwd,
        &provider,
        &model_id,
        args.system_prompt.as_deref(),
        &args.append_system_prompt,
        Some(&agent_mode),
    );

    let context_file_names = build_context_display_names(&resource_loader);
    let skill_names = build_skill_display_names(&resource_loader);
    let home_dir = dirs::home_dir().map(|h| h.to_string_lossy().to_string());

    // Create autocomplete provider
    let autocomplete_provider = build_autocomplete_provider(&cwd, &resource_loader);

    // Compute folder name for terminal title
    let folder = std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
        .unwrap_or_else(|| "Pick".to_string());

    // Create TUI app
    let thinking_str = format!("{:?}", thinking_level).to_lowercase();
    let mut tui = match TuiApp::new(
        &provider,
        &model_id,
        app_name,
        version,
        context_file_names,
        skill_names,
        &cwd.to_string_lossy(),
        home_dir,
        &thinking_str,
        autocomplete_provider,
        &folder,
        &agent_mode.to_string(),
    ) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error starting TUI: {}", e);
            std::process::exit(1);
        }
    };

    tui.set_context_info(Some(0.0), model.context_window);

    // Restore session name
    if let Some(name) = session_manager.get_session_name() {
        tui.set_session_name(name.to_string());
    }
    tui.update_terminal_title();

    // Wire theme colors
    let theme = crate::core::theme::global_theme();
    if let Ok(theme_guard) = theme.lock() {
        let tui_colors = crate::core::theme::theme_to_tui_colors(&theme_guard);
        tui.set_colors(tui_colors);
    }

    // Create terminal manager
    let terminal_manager = match TerminalManager::new() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error initializing terminal: {}", e);
            std::process::exit(1);
        }
    };

    // Add startup header (skip if quiet_startup is enabled)
    let settings = SettingsManager::load(&cwd);
    tui.show_hardware_cursor = settings.get_show_hardware_cursor();
    if !settings.get_quiet_startup() {
        if let Ok((width, _height)) = crossterm::terminal::size() {
            tui.ensure_startup_header(width as usize);
        } else {
            tui.ensure_startup_header(80);
        }
    }

    // Check for update on startup and add banner if available
    #[cfg(not(debug_assertions))]
    {
        let settings = SettingsManager::load(&cwd);
        if settings.get_check_for_update_on_startup() {
            let current_ver = crate::config::VERSION;
            if let Some(latest_ver) = crate::core::updates::get_upgrade_version(true).await {
                if latest_ver != current_ver {
                    let banner = format!(
                        "\x1b[1m\x1b[38;2;100;200;100m\u{2191}\x1b[0m \
                         \x1b[1mUpdate Available:\x1b[0m \
                         Pick v{} \u{2192} v{}  \
                         \x1b[2m(run `pick update` to upgrade)\x1b[0m",
                        current_ver, latest_ver
                    );
                    tui.chat.add_system_message(&banner);
                }
            }
        }
    }

    // Auto-show changelog on version change
    {
        let sm = SettingsManager::load(&cwd);
        let collapse = sm.get_collapse_changelog();
        let last_version = sm.get().last_changelog_version.clone();
        let current_ver = crate::config::VERSION;
        if last_version.as_deref() != Some(current_ver) && last_version.is_some() {
            use crate::utils::changelog::{get_changelog_path, get_new_entries, parse_changelog};
            let changelog_path = get_changelog_path();
            if changelog_path.exists() {
                let entries = parse_changelog(&changelog_path);
                let new_entries =
                    get_new_entries(&entries, last_version.as_deref().unwrap_or("0.0.0"));
                if !new_entries.is_empty() {
                    tui.chat.add_system_message(
                        "\x1b[1m\x1b[38;2;100;200;100m\u{1f4dd} What's New\x1b[0m",
                    );
                    for entry in &new_entries {
                        let version = format!("v{}.{}.{}", entry.major, entry.minor, entry.patch);
                        tui.chat
                            .add_system_message(&format!("\x1b[1m{}\x1b[0m", version));
                        let lines: Vec<&str> = if collapse {
                            // Condensed: only title/header line
                            entry.content.lines().take(1).collect()
                        } else {
                            entry.content.lines().collect()
                        };
                        for line in &lines {
                            tui.chat
                                .add_system_message(&format!("\x1b[2m  {}\x1b[0m", line));
                        }
                        if collapse && entry.content.lines().count() > 1 {
                            tui.chat.add_system_message("\x1b[2m  ...\x1b[0m");
                        }
                        tui.chat.add_system_message("");
                    }
                }
            }
        }
        // Update last_changelog_version to current
        if last_version.as_deref() != Some(current_ver) {
            let mut update = crate::core::settings::Settings::default();
            update.last_changelog_version = Some(current_ver.to_string());
            let mut settings = SettingsManager::load(&cwd);
            let _ = settings.set_global(update);
        }
    }

    // Create channels
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<TuiCommand>();
    let (evt_tx, evt_rx) = mpsc::unbounded_channel::<crossterm::event::Event>();

    // Spawn keyboard reader thread
    std::thread::spawn(move || {
        loop {
            match crossterm::event::read() {
                Ok(event) => {
                    if evt_tx.send(event).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
    });

    // Enable bracketed paste mode
    print!("\x1b[?2004h");
    let _ = std::io::Write::flush(&mut std::io::stdout());

    // Tool tracking state
    let tool_start_times: Arc<Mutex<HashMap<String, Instant>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let tool_args_map: Arc<Mutex<HashMap<String, serde_json::Value>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Load display/behavior flags from settings
    let settings = SettingsManager::load(&cwd);
    let hide_thinking = Arc::new(AtomicBool::new(settings.get_hide_thinking_block()));
    let show_images = Arc::new(AtomicBool::new(settings.get_show_images()));
    let block_images = Arc::new(AtomicBool::new(settings.get_block_images()));
    let steer_mode = match settings.get_steering_mode() {
        "all" => QueueMode::All,
        _ => QueueMode::OneAtATime,
    };
    let follow_up_mode = match settings.get_follow_up_mode() {
        "all" => QueueMode::All,
        _ => QueueMode::OneAtATime,
    };

    // Build on_event callback
    let on_event = event_handler::create_on_event(
        cmd_tx.clone(),
        tool_start_times.clone(),
        tool_args_map.clone(),
        hide_thinking.clone(),
        show_images.clone(),
        block_images.clone(),
    );

    // Restore session history
    let all_messages = message_utils::restore_session_history(
        &mut tui,
        &session_manager,
        &initial_messages,
        hide_thinking.load(Ordering::Relaxed),
        show_images.load(Ordering::Relaxed),
        block_images.load(Ordering::Relaxed),
    );

    let ctx = TuiContext {
        tui,
        terminal_manager,
        cmd_tx,
        all_messages,
        session_manager,
        model,
        provider,
        model_id,
        thinking_level,
        tools,
        system_prompt,
        agent_mode,
        resource_loader,
        extension_runner,
        agent_registry,
        mcp_manager,
        mcp_cancelled,
        permission_manager,
        platform_sandbox,
        auth,
        hide_thinking,
        show_images,
        block_images,
        tool_start_times,
        tool_args_map,
        on_event,
        pending_command: None,
        skill_command_executed: false,
        scoped_models: Vec::new(),
        was_interrupted: Arc::new(AtomicBool::new(false)),
        steer_queue: Arc::new(Mutex::new(PendingMessageQueue::new(steer_mode))),
        follow_up_queue: Arc::new(Mutex::new(PendingMessageQueue::new(follow_up_mode))),
        next_turn_queue: Arc::new(Mutex::new(PendingMessageQueue::new(QueueMode::All))),
        steer_queue_mode: steer_mode,
        follow_up_queue_mode: follow_up_mode,
        agent_is_running: false,
        agent_cancel_tx: None,
        agent_cancel_requested: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        agent_abort_handle: None,
        agent_start_message_count: 0,
        args,
        all_tools,
        cwd,
        version,
        app_name,
        pending_update: None,
    };

    (ctx, cmd_rx, evt_rx)
}

/// Filter tools by agent mode ruleset
fn filter_tools_by_mode(
    all_tools: &Arc<RwLock<Vec<AgentTool>>>,
    agent_mode: &AgentMode,
    session_manager: &SessionManager,
) -> Vec<AgentTool> {
    let ruleset = agent_mode.ruleset();
    let locked = all_tools.read().unwrap();
    pick_agent::tools::filter_goal_tools(
        pick_agent::permission::disabled::filter_tools(locked.clone(), &[&ruleset]),
        session_manager.goal_manager(),
    )
}

/// Re-filter tools after mode change or MCP change
pub(crate) fn refilter_tools(
    all_tools: &Arc<RwLock<Vec<AgentTool>>>,
    agent_mode: &AgentMode,
    session_manager: &SessionManager,
) -> Vec<AgentTool> {
    let ruleset = agent_mode.ruleset();
    pick_agent::tools::filter_goal_tools(
        pick_agent::permission::disabled::filter_tools(
            all_tools.read().unwrap().clone(),
            &[&ruleset],
        ),
        session_manager.goal_manager(),
    )
}

/// Resolve model from args
fn resolve_model(args: &Args) -> (AiModel, String, String) {
    let provider = args.provider.as_deref().unwrap_or("anthropic").to_string();
    let model_id = args
        .model
        .as_deref()
        .unwrap_or("claude-sonnet-4-20250514")
        .to_string();
    let model = get_model(&provider, &model_id);

    match model {
        Some(m) => (m, provider, model_id),
        None => {
            eprintln!(
                "Error: model '{}' not found for provider '{}'",
                model_id, provider
            );
            std::process::exit(1);
        }
    }
}

/// Re-resolve model when model_id or provider changes
pub(crate) fn update_model(provider: &str, model_id: &str) -> (AiModel, String) {
    let new_model = get_model(provider, model_id);
    match new_model {
        Some(m) => (m, provider.to_string()),
        None => {
            let models = pick_ai::models::get_models(provider);
            if let Some(fallback) = models.first() {
                (fallback.clone(), provider.to_string())
            } else {
                panic!("No models available for provider '{}'", provider);
            }
        }
    }
}

/// Set API key env var for a given provider
pub(crate) async fn update_api_key(auth: &AuthStorage, provider: &str) {
    let env_var = format!("{}_API_KEY", provider.to_uppercase().replace('-', "_"));
    if std::env::var(&env_var).is_err()
        && let Some(key) = auth.get_api_key(provider, true).await
    {
        unsafe {
            std::env::set_var(&env_var, key);
        }
    }
}

/// Save model/provider as default in global settings
pub(crate) fn save_default_model(provider: &str, model_id: &str) {
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut sm = SettingsManager::load(&cwd);
    let mut update = crate::core::settings::Settings::default();
    update.default_provider = Some(provider.to_string());
    update.default_model = Some(model_id.to_string());
    let _ = sm.set_global(update);
}

/// Resolve thinking level from CLI arg and model capability
fn resolve_thinking_level(thinking_arg: Option<&str>, model: &AiModel) -> ThinkingLevel {
    match thinking_arg {
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
    }
}

/// Build autocomplete provider with slash commands and skills
fn build_autocomplete_provider(
    cwd: &std::path::Path,
    resource_loader: &ResourceLoader,
) -> Option<Box<dyn pick_tui::autocomplete::AutocompleteProvider>> {
    use crate::core::slash_commands::BUILTIN_SLASH_COMMANDS;
    let mut commands: Vec<TuiSlashCommand> = BUILTIN_SLASH_COMMANDS
        .iter()
        .map(|c| TuiSlashCommand {
            name: c.name.to_string(),
            description: Some(c.description.to_string()),
            argument_hint: None,
        })
        .collect();
    let sm = SettingsManager::load(cwd);
    if sm.get_enable_skill_commands() {
        for skill in resource_loader.skills() {
            commands.push(TuiSlashCommand {
                name: format!("skill:{}", skill.name),
                description: Some(skill.description.clone()),
                argument_hint: None,
            });
        }
    }
    let provider = CombinedAutocompleteProvider::new(commands, cwd.to_path_buf());
    Some(Box::new(provider))
}

/// Rebuild system prompt
pub(crate) fn rebuild_system_prompt(
    tools: &[AgentTool],
    resource_loader: &ResourceLoader,
    cwd: &std::path::Path,
    provider: &str,
    model_id: &str,
    system_prompt_override: Option<&str>,
    append_system_prompt: &[String],
    agent_mode: Option<&AgentMode>,
) -> String {
    build_prompt(
        tools,
        resource_loader,
        cwd,
        provider,
        model_id,
        system_prompt_override,
        append_system_prompt,
        agent_mode,
    )
}
