//! Print mode - non-interactive agent execution with stdout output

use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::args::Args;
use crate::core::auth_storage::AuthStorage;
use pick_agent::core::agent_loop::AgentLoopConfig;
use pick_agent::core::events::{AgentEvent, agent_event_to_json_value};
use pick_agent::core::state::{AgentTool, ThinkingLevel};
use pick_agent::extensions::runner::ExtensionRunner;
use pick_agent::session::{SessionEntry, SessionManager};
use pick_ai::models::get_model;
use pick_ai::types::{ContentBlock, Message, UserMessage};

use crate::core::agent_mode::AgentMode;
use crate::core::system_prompt::build_system_prompt_with_defaults_and_mode;

/// Run the agent in print mode (non-interactive, batch)
pub async fn run_print_mode(
    args: Args,
    tools: Vec<AgentTool>,
    auth: Arc<AuthStorage>,
    session_manager: SessionManager,
    initial_messages: Vec<Message>,
    extension_runner: Option<Arc<ExtensionRunner>>,
    agent_mode: AgentMode,
    agent_registry: Arc<pick_agent::agent_registry::AgentRegistry>,
    permission_manager: Arc<pick_agent::permission::manager::PermissionManager>,
    platform_sandbox: Option<std::sync::Arc<dyn pick_agent::permission::sandbox::Sandbox>>,
    sandbox_enabled: Arc<AtomicBool>,
) {
    // Resolve model + provider
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

    let env_var = format!("{}_API_KEY", provider.to_uppercase().replace('-', "_"));
    let api_key = auth
        .get_api_key(provider, true)
        .await
        .or_else(|| std::env::var(&env_var).ok());
    if api_key.is_none() {
        eprintln!(
            "Error: No API key for '{}'. Set {}_API_KEY.",
            provider, env_var
        );
        std::process::exit(1);
    }
    if std::env::var(&env_var).is_err()
        && let Some(ref key) = api_key
    {
        // SAFETY: set_var is safe in single-threaded context at startup
        unsafe {
            std::env::set_var(&env_var, key);
        }
    }

    let tools = if args.no_tools { vec![] } else { tools };

    let cwd = std::env::current_dir().unwrap_or_default();
    let append_text = if args.append_system_prompt.is_empty() {
        format!("Provider: {}  Model: {}", provider, model_id)
    } else {
        format!(
            "{}\nProvider: {}  Model: {}",
            args.append_system_prompt.join("\n"),
            provider,
            model_id
        )
    };
    let tools = pick_agent::tools::filter_goal_tools(
        pick_agent::permission::disabled::filter_tools(tools, &[&agent_mode.ruleset()]),
        session_manager.goal_manager(),
    );
    let system_prompt = build_system_prompt_with_defaults_and_mode(
        &tools,
        &[],
        &[],
        args.system_prompt.as_deref(),
        Some(&append_text),
        &cwd,
        Some(&agent_mode),
    );

    // Build messages from args and stdin
    let mut messages = Vec::new();
    for msg in &args.messages {
        messages.push(Message::User(UserMessage::text(msg)));
    }

    let stdin_content = read_stdin();
    if let Some(content) = stdin_content {
        messages.push(Message::User(UserMessage::text(&content)));
    }

    // Also include initial_messages from any other source
    messages.extend(initial_messages);

    if messages.is_empty() {
        eprintln!("Error: No input messages provided.");
        std::process::exit(1);
    }

    // Capture output text
    let output = Arc::new(std::sync::Mutex::new(String::new()));
    let output_clone = output.clone();
    let mode_is_json = args.mode == "json";

    // Track persisted message count for incremental session persistence
    let persisted_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let session_mgr = Arc::new(tokio::sync::Mutex::new(session_manager));

    let mode_rules = agent_mode.ruleset();
    let config = AgentLoopConfig {
        model: model.clone(),
        system_prompt: system_prompt.clone(),
        developer_sections: vec![],
        tools,
        thinking_level: ThinkingLevel::Off,
        max_tokens: None,
        temperature: None,
        extension_runner: extension_runner.clone(),
        transform_context: None,
        get_api_key: None,
        on_turn_complete: Some(Arc::new({
            let sm = session_mgr.clone();
            let pc = persisted_count.clone();
            move |messages: &[Message]| {
                let sm = sm.clone();
                let pc = pc.clone();
                let msgs: Vec<Message> = messages.to_vec();
                Box::pin(async move {
                    let prev = pc.load(std::sync::atomic::Ordering::Relaxed);
                    if msgs.len() > prev {
                        let mut guard = sm.lock().await;
                        for msg in &msgs[prev..] {
                            if let Err(e) = guard.append(SessionEntry::from(msg)).await {
                                eprintln!("Warning: session persist failed: {}", e);
                            }
                        }
                        pc.store(msgs.len(), std::sync::atomic::Ordering::Relaxed);
                    }
                })
            }
        })),
        mode_rulesets: Some(vec![mode_rules.clone()]),
        before_tool_call: Some(std::sync::Arc::new(
            move |tc: &pick_ai::types::ToolCall| -> Option<String> {
                let tool_args_str = tc.arguments.to_string();
                pick_agent::permission::evaluate::check_permission(
                    &tc.name,
                    &tool_args_str,
                    &[&mode_rules],
                )
                .err()
            },
        )),
        should_stop_after_turn: None,
        get_steering_messages: Some(Arc::new({
            let mode = agent_mode;
            move || match mode {
                AgentMode::Plan => {
                    vec![Message::User(UserMessage::text(
                        crate::core::agent_mode::PLAN_MODE_REMINDER,
                    ))]
                }
                AgentMode::Build => vec![],
            }
        })),
        get_follow_up_messages: None,
        provider_max_retries: None,
        provider_max_retry_delay_ms: None,
        approve: None,
        question: None,
        agent_id: None,
        agent_registry: Some(agent_registry.clone()),
        fs_policy: permission_manager.fs_policy(),
        cwd: Some(std::env::current_dir().unwrap_or_default()),
        permission_hooks: Some(permission_manager.hook_registry.clone()),
        permission_manager: Some(permission_manager.clone()),
        tool_event_bus: None,
        sandbox: platform_sandbox.clone(),
        sandbox_enabled: Some(sandbox_enabled.clone()),
        cancel_signal_tx: None,
        skill_paths: Vec::new(),
        parent_goal_manager: None,
        on_event: Some(Arc::new(move |event| {
            if mode_is_json {
                let json_line = agent_event_to_json_value(&event);
                if let Ok(line) = serde_json::to_string(&json_line) {
                    use std::io::Write;
                    let _ = std::io::stdout().lock().write_all(line.as_bytes());
                    let _ = std::io::stdout().lock().write_all(b"\n");
                    let _ = std::io::stdout().lock().flush();
                }
            }
            if let AgentEvent::MessageUpdate { message, .. } = event
                && let Message::Assistant(msg) = message
            {
                for block in &msg.content {
                    if let ContentBlock::Text(t) = block {
                        let mut out = output_clone.lock().unwrap();
                        out.push_str(&t.text);
                    }
                }
            }
        })),
    };

    match crate::core::agent_session::run_agent_loop_with_retry_and_continuation(
        config,
        messages,
        Default::default(),
        None,
    )
    .await
    {
        Ok(result) => {
            // Flush any remaining messages not yet persisted by on_turn_complete
            let prev = persisted_count.load(std::sync::atomic::Ordering::Relaxed);
            if result.messages.len() > prev {
                let mut guard = session_mgr.lock().await;
                for msg in &result.messages[prev..] {
                    if let Err(e) = guard.append(SessionEntry::from(msg)).await {
                        eprintln!("Warning: session persist failed: {}", e);
                    }
                }
            }

            if args.mode == "json" {
                // Events already streamed via on_event callback.
                // Output agent_end as final event.
                let end_event = serde_json::json!({
                    "type": "agent_end",
                    "messages": result.messages,
                });
                if let Ok(line) = serde_json::to_string(&end_event) {
                    use std::io::Write;
                    let _ = std::io::stdout().lock().write_all(line.as_bytes());
                    let _ = std::io::stdout().lock().write_all(b"\n");
                    let _ = std::io::stdout().lock().flush();
                }
            } else {
                // Print collected output
                let text = output.lock().unwrap().clone();
                if !text.is_empty() {
                    println!("{}", text);
                }

                // Fallback: print final message content directly
                if text.is_empty() {
                    for msg in &result.messages {
                        if let Message::Assistant(am) = msg {
                            for block in &am.content {
                                if let ContentBlock::Text(t) = block {
                                    println!("{}", t.text);
                                }
                            }
                        }
                    }
                }

                eprintln!(
                    "\n[Input: {} | Output: {} | Cache R/W: {}/{}]",
                    result.usage.input,
                    result.usage.output,
                    result.usage.cache_read,
                    result.usage.cache_write,
                );
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

/// Read content from stdin if available
fn read_stdin() -> Option<String> {
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();
    let mut content = String::new();
    match handle.read_to_string(&mut content) {
        Ok(0) => None,
        Ok(_) => {
            let trimmed = content.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        Err(_) => None,
    }
}
