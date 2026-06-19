use std::io::Write;
use std::sync::Arc;

use tokio::io::AsyncBufReadExt;
use tokio::sync::watch;

use crate::args::Args;
use crate::core::auth_storage::AuthStorage;
use pick_agent::core::agent_loop::AgentLoopConfig;
use pick_agent::core::events::AgentEvent;
use pick_agent::core::state::{AgentTool, ThinkingLevel};
use pick_agent::extensions::runner::ExtensionRunner;
use pick_agent::session::{SessionEntry, SessionManager};
use pick_ai::models::get_model;
use pick_ai::types::{ContentBlock, Message, UserMessage};

use crate::core::agent_mode::AgentMode;
use crate::core::agent_session::RetryConfig;
use crate::core::system_prompt::build_system_prompt_with_defaults;

struct RpcContext {
    model_id: String,
    provider: String,
    system_prompt: String,
    tools: Vec<AgentTool>,
    messages: Vec<Message>,
    session_manager: SessionManager,
    extension_runner: Option<Arc<ExtensionRunner>>,
    agent_mode: AgentMode,
    auto_retry_enabled: bool,
    cancel_tx: Option<watch::Sender<bool>>,
}

/// Run the agent in RPC mode (JSON-RPC over stdin/stdout)
pub async fn run_rpc_mode(
    args: Args,
    tools: Vec<AgentTool>,
    auth: Arc<AuthStorage>,
    session_manager: SessionManager,
    extension_runner: Option<Arc<ExtensionRunner>>,
    agent_mode: AgentMode,
    agent_registry: Arc<pick_agent::agent_registry::AgentRegistry>,
    permission_manager: Arc<pick_agent::permission::manager::PermissionManager>,
    platform_sandbox: Option<std::sync::Arc<dyn pick_agent::permission::sandbox::Sandbox>>,
) {
    let provider = args.provider.as_deref().unwrap_or("anthropic").to_string();
    let model_id = args
        .model
        .as_deref()
        .unwrap_or("claude-sonnet-4-20250514")
        .to_string();

    let env_var = format!("{}_API_KEY", provider.to_uppercase().replace('-', "_"));
    let api_key = auth
        .get_api_key(&provider, true)
        .await
        .or_else(|| std::env::var(&env_var).ok());
    if let Some(ref key) = api_key
        && std::env::var(&env_var).is_err() {
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
    let system_prompt = build_system_prompt_with_defaults(
        &tools,
        &[],
        &[],
        args.system_prompt.as_deref(),
        Some(&append_text),
        &cwd,
    );

    let mut ctx = RpcContext {
        model_id,
        provider,
        system_prompt,
        tools,
        messages: Vec::new(),
        session_manager,
        extension_runner,
        agent_mode,
        auto_retry_enabled: true,
        cancel_tx: None,
    };

    for msg in &args.messages {
        ctx.messages.push(Message::User(UserMessage::text(msg)));
    }

    eprintln!("RPC mode started. Listening for JSON-RPC messages on stdin...");

    let stdin = tokio::io::stdin();
    let reader = tokio::io::BufReader::new(stdin);
    let mut lines = reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        if let Ok(request) = serde_json::from_str::<serde_json::Value>(&line) {
            let method = request
                .get("method")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown");
            let id = request.get("id");
            let params = request.get("params");

            match method {
                "ping" => {
                    send_response(id, serde_json::json!("pong"));
                }
                "echo" => {
                    send_response(id, params.cloned().unwrap_or(serde_json::Value::Null));
                }
                "ask" | "chat" | "generate" => {
                    let prompt = params
                        .and_then(|p| p.get("prompt"))
                        .or(params)
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    let msg = Message::User(UserMessage::text(prompt));
                    let msgs_before_submit = ctx.messages.len();
                    ctx.messages.push(msg.clone());

                    let model = match get_model(&ctx.provider, &ctx.model_id) {
                        Some(m) => m,
                        None => {
                            send_error(id, -32603, format!("Model '{}' not found", ctx.model_id));
                            continue;
                        }
                    };

                    let response_text = Arc::new(std::sync::Mutex::new(String::new()));
                    let rt = response_text.clone();

                    let (cancel_tx, cancel_rx) = watch::channel(false);
                    ctx.cancel_tx = Some(cancel_tx);
                    let cancel_rx = Arc::new(cancel_rx);

                    let retry_config = RetryConfig {
                        enabled: ctx.auto_retry_enabled,
                        ..Default::default()
                    };

                    let config = AgentLoopConfig {
                        model: model.clone(),
                        system_prompt: ctx.system_prompt.clone(),
                        tools: ctx.tools.clone(),
                        thinking_level: ThinkingLevel::Off,
                        max_tokens: None,
                        temperature: None,
                        extension_runner: ctx.extension_runner.clone(),
                        transform_context: None,
                        get_api_key: None,
                        before_tool_call: None,
                        should_stop_after_turn: None,
                        get_steering_messages: Some(Arc::new({
                            let mode = ctx.agent_mode;
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
                        on_event: Some(Arc::new(move |event| {
                            if let AgentEvent::MessageUpdate { message, .. } = event
                                && let Message::Assistant(msg) = message {
                                    for block in &msg.content {
                                        if let ContentBlock::Text(t) = block {
                                            rt.lock().unwrap().push_str(&t.text);
                                        }
                                    }
                                }
                        })),
                        fs_policy: permission_manager.fs_policy(),
                        cwd: Some(std::env::current_dir().unwrap_or_default()),
                        mode_rulesets: None,
                        permission_hooks: Some(permission_manager.hook_registry.clone()),
                        permission_manager: Some(permission_manager.clone()),
                        sandbox: platform_sandbox.clone(),
                        on_turn_complete: None,
                    };

                    let msgs = ctx.messages.clone();

                    let agent_handle = tokio::spawn(async move {
                        crate::core::agent_session::run_agent_loop_with_retry_and_continuation(
                            config,
                            msgs,
                            retry_config,
                            Some(cancel_rx),
                        )
                        .await
                    });

                    let result = tokio::select! {
                        result = agent_handle => {
                            match result {
                                Ok(r) => r,
                                Err(e) => Err(format!("Agent task failed: {}", e)),
                            }
                        }
                        _ = tokio::signal::ctrl_c() => {
                            ctx.messages.truncate(msgs_before_submit);
                            Err("Interrupted".to_string())
                        }
                    };

                    ctx.cancel_tx = None;

                    match result {
                        Ok(agent_result) => {
                            for msg in &agent_result.messages[msgs_before_submit..] {
                                if let Err(e) =
                                    ctx.session_manager.append(SessionEntry::from(msg)).await
                                {
                                    eprintln!("Warning: session persist failed: {}", e);
                                }
                            }
                            ctx.messages = agent_result.messages;
                            let text = response_text.lock().unwrap().clone();
                            send_response(
                                id,
                                serde_json::json!({
                                    "content": text,
                                    "usage": {
                                        "input": agent_result.usage.input,
                                        "output": agent_result.usage.output,
                                    }
                                }),
                            );
                        }
                        Err(e) => {
                            send_error(id, -32603, e);
                        }
                    }
                }
                "abort_retry" => {
                    if let Some(ref tx) = ctx.cancel_tx {
                        let _ = tx.send(true);
                    }
                    send_response(id, serde_json::json!({"success": true}));
                }
                "set_auto_retry" => {
                    if let Some(enabled) =
                        params.and_then(|p| p.get("enabled").and_then(|v| v.as_bool()))
                    {
                        ctx.auto_retry_enabled = enabled;
                    }
                    send_response(id, serde_json::json!({"success": true}));
                }
                "history" => {
                    let history: Vec<serde_json::Value> = ctx.messages.iter().map(|m| match m {
                        Message::User(u) => {
                            let text: String = u.content.iter()
                                .filter_map(|c| match c {
                                    ContentBlock::Text(t) => Some(t.text.clone()),
                                    _ => None,
                                })
                                .collect();
                            serde_json::json!({"role": "user", "content": text})
                        }
                        Message::Assistant(a) => {
                            let text: String = a.content.iter()
                                .filter_map(|c| match c {
                                    ContentBlock::Text(t) => Some(t.text.clone()),
                                    _ => None,
                                })
                                .collect();
                            serde_json::json!({"role": "assistant", "content": text})
                        }
                        Message::ToolResult(t) => serde_json::json!({"role": "tool", "tool_call_id": t.tool_call_id, "tool_name": t.tool_name}),
                    }).collect();
                    send_response(id, serde_json::json!(history));
                }
                "clear_history" => {
                    ctx.messages.clear();
                    send_response(id, serde_json::json!(true));
                }
                "model" => {
                    send_response(
                        id,
                        serde_json::json!({
                            "provider": ctx.provider,
                            "model": ctx.model_id,
                        }),
                    );
                }
                "exit" | "shutdown" => {
                    send_response(id, serde_json::json!("bye"));
                    break;
                }
                _ => {
                    send_error(id, -32601, format!("Method not found: {}", method));
                }
            }
        } else {
            send_error(
                None,
                -32700,
                "Parse error: invalid JSON-RPC request".to_string(),
            );
        }

        std::io::stdout().flush().ok();
    }

    eprintln!("RPC mode terminated.");
}

fn send_response(id: Option<&serde_json::Value>, result: serde_json::Value) {
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "result": result,
        "id": id,
    });
    println!("{}", serde_json::to_string(&response).unwrap());
}

fn send_error(id: Option<&serde_json::Value>, code: i32, message: String) {
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "error": {
            "code": code,
            "message": message,
        },
        "id": id,
    });
    println!("{}", serde_json::to_string(&response).unwrap());
}
