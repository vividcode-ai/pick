use std::collections::HashMap;

use tauri::Manager;
use tokio::runtime::Runtime;

struct ServerState {
    pub port: u16,
}

#[allow(dead_code)]
struct ServerRuntime(Runtime);

#[tauri::command]
fn get_server_url(state: tauri::State<ServerState>) -> Option<String> {
    Some(format!("http://127.0.0.1:{}", state.port))
}

#[tauri::command]
fn get_os_info() -> String {
    std::env::consts::OS.to_string()
}

#[cfg(desktop)]
fn run_desktop() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_ansi(false)
        .init();

    tauri::Builder::default()
        .setup(|app| {
            let rt = Runtime::new().expect("Failed to create tokio runtime");

            // Load API keys and last used model from auth.json
            let (api_keys, last_provider, last_model, thinking_level) = load_auth_credentials();

            let port = rt.block_on(async {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                    .await
                    .expect("Failed to bind to localhost");
                let port = listener.local_addr().unwrap().port();

                let config = pick_server::ServerConfig {
                    host: "127.0.0.1".to_string(),
                    port,
                    cwd: None,
                    auth_storage_path: None,
                    api_keys,
                    last_provider,
                    last_model,
                    thinking_level,
                };

                tokio::spawn(async move {
                    if let Err(e) = pick_server::run_server_on_listener(listener, config).await {
                        eprintln!("pick-server error: {}", e);
                    }
                });

                port
            });

            app.manage(ServerState { port });
            app.manage(ServerRuntime(rt));

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_server_url, get_os_info])
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(mobile)]
fn run_mobile() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_os_info])
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Load API credentials and last used model info from ~/.pick/agent/auth.json.
fn load_auth_credentials() -> (
    HashMap<String, String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let auth_path = pick_agent::auth::default_auth_path();
    if auth_path.exists() {
        if let Ok(file) = pick_agent::auth::read_auth_file(&auth_path) {
            let keys = file
                .credentials
                .into_iter()
                .filter_map(|(provider, cred)| match cred {
                    pick_agent::auth::AuthCredential::ApiKey { key } => Some((provider, key)),
                    _ => None,
                })
                .collect();
            return (
                keys,
                file.last_provider,
                file.last_model,
                file.thinking_level,
            );
        }
    }
    (HashMap::new(), None, None, None)
}

pub fn run() {
    #[cfg(desktop)]
    {
        run_desktop();
    }

    #[cfg(mobile)]
    {
        run_mobile();
    }
}
