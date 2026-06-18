use std::sync::Arc;

use pick_tui::components::select::{SelectItem, SelectList};
use tokio::sync::oneshot;

use super::context::TuiContext;
use super::init;

/// Handle login selection: apikey or subscription
pub(crate) async fn handle_login_selection(ctx: &mut TuiContext, val: &str) {
    match val {
        "apikey" => {
            ctx.pending_command = Some("login-apikey".to_string());
            let providers = pick_ai::models::get_providers();
            let items: Vec<SelectItem> = providers
                .iter()
                .map(|p| {
                    let label = match p.as_str() {
                        "anthropic" => "Anthropic",
                        "openai" => "OpenAI",
                        "deepseek" => "DeepSeek",
                        "google" => "Google (Gemini)",
                        "groq" => "Groq",
                        "mistral" => "Mistral",
                        "xai" => "xAI",
                        "openrouter" => "OpenRouter",
                        "github-copilot" => "GitHub Copilot",
                        "fireworks" => "Fireworks",
                        "together" => "Together",
                        "cerebras" => "Cerebras",
                        "huggingface" => "HuggingFace",
                        "amazon-bedrock" => "Amazon Bedrock",
                        "google-vertex" => "Google Vertex",
                        _ => p.as_str(),
                    };
                    SelectItem::new(label.to_string(), p.clone())
                        .with_description("Sign in with API key")
                })
                .collect();
            if items.is_empty() {
                ctx.tui
                    .chat
                    .add_system_message("No providers available for API key login.");
            } else {
                let select = SelectList::new("Select Provider", items);
                ctx.tui.start_selection(select);
                ctx.tui.finalize_turn();
            }
        }
        "subscription" => {
            ctx.pending_command = Some("login-oauth".to_string());
            let providers = pick_ai::oauth::list_oauth_providers();
            let items: Vec<SelectItem> = providers
                .iter()
                .map(|p| {
                    let label = match p.as_str() {
                        "anthropic" => "Anthropic",
                        "github-copilot" => "GitHub Copilot",
                        "openai-codex" => "OpenAI Codex",
                        _ => p.as_str(),
                    };
                    SelectItem::new(label.to_string(), p.clone())
                        .with_description("Sign in with OAuth")
                })
                .collect();
            let select = SelectList::new("Select OAuth Provider", items);
            ctx.tui.start_selection(select);
            ctx.tui.finalize_turn();
        }
        _ => {
            ctx.tui.chat.add_system_message("Unknown login method.");
        }
    }
}

/// Handle OAuth login flow
pub(crate) async fn handle_oauth_login(ctx: &mut TuiContext, provider_id: &str) {
    let provider_id_display = provider_id.to_string();
    let auth_clone = Arc::clone(&ctx.auth);

    ctx.tui.chat.add_system_message(&format!(
        "Starting OAuth login for \x1b[1m{}\x1b[0m...",
        provider_id_display
    ));

    let (msg_tx, mut msg_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let (result_tx, mut result_rx) = oneshot::channel::<Result<(), String>>();

    let auth_url_tx = msg_tx.clone();
    let device_code_tx = msg_tx.clone();
    let success_tx = msg_tx.clone();

    let pid = provider_id.to_string();
    tokio::spawn(async move {
        let provider = match pick_ai::oauth::get_oauth_provider(&pid) {
            Some(p) => p,
            None => {
                let _ = result_tx.send(Err(format!("Unknown OAuth provider: {}", pid)));
                return;
            }
        };

        let callbacks = pick_ai::oauth::OAuthLoginCallbacks {
            on_auth_url: Box::new(move |url: &str| {
                let _ = auth_url_tx.send(format!(
                    "Open this URL in your browser:\n\x1b[1m{}\x1b[0m",
                    url
                ));
            }),
            on_device_code: Box::new(move |code: &str, url: &str| {
                let _ = device_code_tx.send(format!(
                    "Enter the code \x1b[1m{}\x1b[0m at \x1b[1m{}\x1b[0m\nWaiting for authorization...",
                    code, url
                ));
            }),
            on_prompt: Box::new(|_msg: &str| None),
            on_select: Box::new(
                |_title: &str, _options: &[pick_ai::oauth::OAuthSelectOption]| None,
            ),
            signal: None,
        };

        match provider.login(&callbacks).await {
            Ok(credentials) => {
                let api_key = match provider.get_api_key(&credentials) {
                    Some(k) => k,
                    None => {
                        let _ =
                            result_tx.send(Err("No API key from OAuth credentials".to_string()));
                        return;
                    }
                };
                auth_clone.set_api_key(&pid, &api_key);
                auth_clone.set_runtime_api_key(&pid, api_key.clone());
                let _ = success_tx.send("Credentials saved.".to_string());
                let _ = result_tx.send(Ok(()));
            }
            Err(e) => {
                let _ = result_tx.send(Err(format!("OAuth login failed: {}", e)));
            }
        }
    });

    // Process OAuth messages and wait for result
    let mut oauth_done = false;
    while !oauth_done {
        tokio::select! {
            biased;
            msg = msg_rx.recv() => {
                if let Some(text) = msg {
                    ctx.tui.chat.add_system_message(&text);
                }
            }
            result = &mut result_rx => {
                match result {
                    Ok(Ok(())) => {
                        ctx.tui.chat.add_system_message(&format!(
                            "Successfully logged in to \x1b[1m{}\x1b[0m!",
                            provider_id_display
                        ));
                    }
                    Ok(Err(e)) => ctx.tui.show_error(&e),
                    Err(_) => ctx.tui.show_error("Login cancelled"),
                }
                oauth_done = true;
            }
        }
        if ctx
            .tui
            .render_with_terminal(&mut ctx.terminal_manager)
            .is_err()
        {
            oauth_done = true;
        }
    }
}

/// Handle API key login: switch to API key input state
pub(crate) fn handle_apikey_login(ctx: &mut TuiContext, provider_id: &str) {
    ctx.tui.api_key_provider = Some(provider_id.to_string());
    ctx.tui.api_key_input = String::new();
    ctx.tui.state = pick_tui::app::AppState::ApiKeyInput;
}

/// Handle API key submit: save key and switch to provider's model
pub(crate) async fn handle_api_key_submit(ctx: &mut TuiContext, key: &str) {
    let provider_id = ctx
        .tui
        .api_key_provider
        .as_deref()
        .unwrap_or("unknown")
        .to_string();

    ctx.auth.set_api_key(&provider_id, key);
    ctx.auth.set_runtime_api_key(&provider_id, key.to_string());
    let env_var = format!("{}_API_KEY", provider_id.to_uppercase().replace('-', "_"));
    unsafe {
        std::env::set_var(&env_var, key);
    }

    let models = pick_ai::models::get_models(&provider_id);
    if let Some(first_model) = models.first() {
        let (new_model, _) = init::update_model(&provider_id, &first_model.id);
        ctx.model = new_model;
        ctx.model_id = first_model.id.clone();
        ctx.provider = provider_id.clone();
        init::save_default_model(&ctx.provider, &ctx.model_id);
        ctx.tui.model_id = ctx.model_id.clone();
        ctx.tui.provider = ctx.provider.clone();
        ctx.tui.chat.add_system_message(&format!(
            "Successfully saved API key for \x1b[1m{}\x1b[0m. Switched to model: \x1b[1m{} ({})\x1b[0m",
            provider_id, ctx.model_id, ctx.provider
        ));
    } else {
        ctx.tui.chat.add_system_message(&format!(
            "Successfully saved API key for \x1b[1m{}\x1b[0m.",
            provider_id
        ));
    }

    ctx.tui.state = pick_tui::app::AppState::Input;
    ctx.tui.api_key_input.clear();
    ctx.tui.api_key_provider = None;
    ctx.tui.finalize_turn();
}

/// Handle logout: remove credentials for selected provider
pub(crate) fn handle_logout(ctx: &mut TuiContext, val: &str) {
    ctx.auth.remove(val);
    ctx.tui
        .chat
        .add_system_message(&format!("Removed credentials for \x1b[1m{}\x1b[0m.", val));
}
