use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::args::Args;
use ratatui::prelude::Stylize;

pub async fn run_serve_mode(
    args: Args,
    default_provider: Option<String>,
    default_model: Option<String>,
    api_keys: HashMap<String, String>,
) {
    let host = args.host.unwrap_or_else(|| "127.0.0.1".to_string());
    let bind_addr = if let Some(port) = args.port {
        format!("{}:{}", host, port)
    } else {
        format!("{}:0", host)
    };

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("Failed to bind address");
    let actual_port = listener.local_addr().unwrap().port();

    // Set TERM for child PTY processes before any ws connections
    // Safety: called once at startup before any concurrent threads access env
    unsafe {
        std::env::set_var("TERM", "xterm-256color");
    }

    let mut state = pick_server::AppState::new(pick_server::ServerConfig {
        host: host.clone(),
        port: actual_port,
        cwd: None,
        auth_storage_path: Some(
            pick_agent::auth::default_auth_path()
                .to_string_lossy()
                .to_string(),
        ),
    });
    state.default_provider = default_provider;
    state.default_model = default_model;
    *state.api_keys.write().unwrap() = api_keys;

    // Load persisted sessions from disk
    let cwd = std::env::current_dir().unwrap_or_default();
    state.load_persisted_sessions(&cwd).await;

    let state = Arc::new(state);

    // Load MCP servers from settings in background (don't block startup)
    let state_for_mcp = state.clone();
    let cwd_for_mcp = cwd.clone();
    tokio::spawn(async move {
        state_for_mcp.load_mcp_from_settings(&cwd_for_mcp).await;
    });

    if args.open_browser {
        let url = format!("http://{}:{}", host, actual_port);
        let url_clone = url.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let _ = open::that(&url_clone);
        });
        println!("Opening {} in browser...", url);
    }

    println!(
        "{} {} {}",
        "Pick".green().bold(),
        "server is running".bold(),
        "▶".green()
    );
    println!(
        "  {}  http://{}:{}",
        "Local".bold(),
        host,
        actual_port.cyan()
    );
    println!(
        "  {} http://{}:{}",
        "Web UI".bold(),
        host,
        actual_port.cyan()
    );
    println!("  Press Ctrl+C to stop");

    pick_server::serve_with_state(listener, state)
        .await
        .expect("Server error");
}
