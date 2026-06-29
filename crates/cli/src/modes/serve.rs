use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::args::Args;

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

    let state = Arc::new(pick_server::AppState {
        session_manager: pick_server::session::SessionManager::new(),
        config: pick_server::ServerConfig {
            host: host.clone(),
            port: actual_port,
        },
        default_provider,
        default_model,
        api_keys,
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

    println!("Pick server listening on ws://{}:{}", host, actual_port);
    println!("  Web UI:  http://{}:{}", host, actual_port);

    pick_server::serve_with_state(listener, state)
        .await
        .expect("Server error");
}
