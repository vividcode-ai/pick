use crate::core::auth_storage::AuthStorage;
use pick_ai::oauth::{get_oauth_provider, list_oauth_providers, OAuthLoginCallbacks};

pub async fn handle_oauth_login(auth: &AuthStorage) {
    let providers = list_oauth_providers();
    if providers.is_empty() {
        println!("No OAuth providers available.");
        return;
    }

    println!("\nAvailable OAuth providers:");
    for (i, id) in providers.iter().enumerate() {
        let status = if auth.get_api_key(id, true).await.is_some() {
            " \x1b[32m(configured)\x1b[0m"
        } else {
            ""
        };
        println!("  {}. {}{}", i + 1, id, status);
    }
    println!("  {}. Cancel", providers.len() + 1);
    print!("Select provider (1-{}): ", providers.len() + 1);
    use std::io::Write;
    std::io::stdout().flush().ok();

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() {
        return;
    }
    let selection: usize = match input.trim().parse() {
        Ok(n) if n >= 1 && n <= providers.len() + 1 => n,
        _ => {
            println!("Invalid selection.");
            return;
        }
    };
    if selection == providers.len() + 1 {
        println!("Cancelled.");
        return;
    }

    let provider_id = &providers[selection - 1];
    println!("\nStarting OAuth login for \x1b[1m{}\x1b[0m...", provider_id);

    let oauth_provider = match get_oauth_provider(provider_id) {
        Some(p) => p,
        None => {
            println!("Error: Provider '{}' not available.", provider_id);
            return;
        }
    };

    let on_auth_url = |url: &str| {
        println!("\nOpen this URL in your browser to authorize:");
        println!("\x1b[34m{}\x1b[0m", url);
    };

    let on_device_code = |user_code: &str, verification_uri: &str| {
        println!("\nEnter the following code at the verification URL:");
        println!("Code: \x1b[1m\x1b[33m{}\x1b[0m", user_code);
        println!("URL:  \x1b[34m{}\x1b[0m", verification_uri);
    };

    let on_prompt_fn = |prompt: &str| -> Option<String> {
        println!("{}", prompt);
        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Ok(_) => {
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() { None } else { Some(trimmed) }
            }
            Err(_) => None,
        }
    };

    let on_select_fn = |_prompt: &str, _options: &[_]| -> Option<String> {
        println!("Enter selection:");
        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Ok(_) => {
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() { None } else { Some(trimmed) }
            }
            Err(_) => None,
        }
    };

    let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);

    let callbacks = OAuthLoginCallbacks {
        on_auth_url: Box::new(on_auth_url),
        on_device_code: Box::new(on_device_code),
        on_prompt: Box::new(on_prompt_fn),
        on_select: Box::new(on_select_fn),
        signal: Some(cancel_rx),
    };

    match oauth_provider.login(&callbacks).await {
        Ok(credentials) => {
            if let Some(api_key) = oauth_provider.get_api_key(&credentials) {
                auth.set_api_key(provider_id, &api_key);
                println!("\n\x1b[32mSuccessfully logged in to {}!\x1b[0m", provider_id);
                println!("Credentials saved to auth file.");
            } else {
                println!("\n\x1b[31mError: No API key in OAuth credentials.\x1b[0m");
            }
        }
        Err(e) => {
            println!("\n\x1b[31mLogin failed: {}\x1b[0m", e);
        }
    }
}
