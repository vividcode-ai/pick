use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
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

/// Open a native directory picker dialog.
/// Returns the selected path as a String, or None if cancelled.
#[tauri::command]
async fn pick_directory(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tokio::sync::oneshot;
    let (tx, rx) = oneshot::channel();
    app.dialog().file().pick_folder(move |f| {
        let _ = tx.send(
            f.and_then(|p| p.into_path().ok())
                .map(|p| p.to_string_lossy().to_string()),
        );
    });
    rx.await.map_err(|_| "Dialog cancelled".to_string())
}

/// Load last used provider/model/thinking from ~/.pick/agent/auth.json.
fn load_last_used() -> (Option<String>, Option<String>, Option<String>) {
    let auth_path = pick_agent::auth::default_auth_path();
    if auth_path.exists()
        && let Ok(file) = pick_agent::auth::read_auth_file(&auth_path)
    {
        return (file.last_provider, file.last_model, file.thinking_level);
    }
    (None, None, None)
}

#[cfg(desktop)]
fn run_desktop() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_ansi(false)
        .init();

    let (last_provider, last_model, thinking_level) = load_last_used();

    tauri::Builder::default()
        .setup(|app| {
            let rt = Runtime::new().expect("Failed to create tokio runtime");

            let port = rt.block_on(async {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                    .await
                    .expect("Failed to bind to localhost");
                let port = listener.local_addr().unwrap().port();

                // api_keys are loaded automatically by AppState::new() from
                //  1) auth.json (~/.pick/agent/auth.json)
                //  2) environment variables (ANTHROPIC_API_KEY, etc.)
                let config = pick_server::ServerConfig {
                    host: "127.0.0.1".to_string(),
                    port,
                    cwd: None,
                    auth_storage_path: Some(
                        pick_agent::auth::default_auth_path()
                            .to_string_lossy()
                            .to_string(),
                    ),
                    api_keys: std::collections::HashMap::new(),
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
                let _ = window.maximize();
                let _ = window.show();
                let _ = window.set_focus();
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_server_url,
            get_os_info,
            pick_directory
        ])
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
