//! Session management service

use std::path::PathBuf;

use crate::args::Args;
use pick_agent::session::SessionManager;

/// Create a session manager based on CLI arguments
pub async fn create_session_manager(
    args: &Args,
    cwd: &PathBuf,
    session_dir_override: Option<PathBuf>,
) -> Result<SessionManager, String> {
    let project_dir = session_dir_override.unwrap_or_else(|| cwd.join(".pick").join("sessions"));
    let global_dir = crate::config::get_sessions_dir();
    let session_dir = Some(project_dir.clone());

    if args.no_session {
        // For in-memory sessions, we create but don't persist
        // Fall through to create
    }

    // --fork: create a new session as a fork of an existing one
    if let Some(ref fork_id) = args.fork {
        return fork_session(fork_id, cwd, &project_dir, &global_dir).await;
    }

    if let Some(ref session_id) = args.session {
        // Try exact match in project dir first, then global dir, then partial search
        let path_candidates = [
            session_dir
                .as_ref()
                .unwrap()
                .join(format!("{}.jsonl", session_id)),
            global_dir.join(format!("{}.jsonl", session_id)),
        ];
        for path in &path_candidates {
            if path.exists() {
                return SessionManager::open(path.clone(), cwd.clone())
                    .await
                    .map_err(|e| format!("Failed to open session: {}", e));
            }
        }
        // Fallback: recursive partial match in both dirs
        if let Some(found) = find_session_file(session_id, &project_dir)
            .or_else(|| find_session_file(session_id, &global_dir))
        {
            return SessionManager::open(found.clone(), cwd.clone())
                .await
                .map_err(|e| format!("Failed to open session: {}", e));
        }
        return Err(format!("Session not found: {}", session_id));
    }

    if args.resume {
        // Try TUI session picker first (when terminal is available)
        if let Some(selected_path) = try_tui_session_picker(cwd, &project_dir).await {
            return SessionManager::open(PathBuf::from(selected_path), cwd.clone())
                .await
                .map_err(|e| format!("Failed to open session: {}", e));
        }

        // User cancelled the session picker
        return Err("cancelled".to_string());
    }

    if let Some(ref continue_val) = args.r#continue {
        if continue_val.is_empty() {
            // -c without ID: scan both project and global dirs, pick newest
            let mut all: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
            for dir in [&project_dir, &global_dir] {
                if dir.exists()
                    && let Ok(entries) = std::fs::read_dir(dir)
                {
                    for e in entries.flatten() {
                        let p = e.path();
                        if p.extension().is_some_and(|ext| ext == "jsonl")
                            && let Ok(meta) = p.metadata()
                            && let Ok(t) = meta.modified()
                        {
                            all.push((p, t));
                        }
                    }
                }
            }
            all.sort_by_key(|(_, t)| *t);
            if let Some((latest, _)) = all.last() {
                return SessionManager::open(latest.clone(), cwd.clone())
                    .await
                    .map_err(|e| format!("Failed to open session: {}", e));
            }
        } else {
            // -c <ID>: search for session by ID (same logic as --session)
            let path_candidates = [
                project_dir.join(format!("{}.jsonl", continue_val)),
                global_dir.join(format!("{}.jsonl", continue_val)),
            ];
            for path in &path_candidates {
                if path.exists() {
                    return SessionManager::open(path.clone(), cwd.clone())
                        .await
                        .map_err(|e| format!("Failed to open session: {}", e));
                }
            }
            if let Some(found) = find_session_file(continue_val, &project_dir)
                .or_else(|| find_session_file(continue_val, &global_dir))
            {
                return SessionManager::open(found.clone(), cwd.clone())
                    .await
                    .map_err(|e| format!("Failed to open session: {}", e));
            }
            return Err(format!("Session not found: {}", continue_val));
        }
    }

    // Create new session
    SessionManager::create(cwd.clone(), session_dir, None, None)
        .await
        .map_err(|e| format!("Failed to create session: {}", e))
}

/// Resolve a fork ID/path and call fork_from on the session manager
async fn fork_session(
    fork_id: &str,
    cwd: &PathBuf,
    project_dir: &PathBuf,
    global_dir: &PathBuf,
) -> Result<SessionManager, String> {
    // If the fork_id ends with .jsonl, treat it as a direct path
    let source_path = if fork_id.ends_with(".jsonl") {
        PathBuf::from(fork_id)
    } else {
        // Try project dir first, then global dir
        let project_path = project_dir.join(format!("{}.jsonl", fork_id));
        if project_path.exists() {
            project_path
        } else {
            // Search for the session ID in both dirs
            let global_path = global_dir.join(format!("{}.jsonl", fork_id));
            if global_path.exists() {
                global_path
            } else {
                // Search recursively in global sessions dir
                find_session_file(fork_id, global_dir)
                    .ok_or_else(|| format!("Session '{}' not found for forking", fork_id))?
            }
        }
    };

    SessionManager::fork_from(source_path, cwd.clone(), None, None)
        .await
        .map_err(|e| format!("Failed to fork session: {}", e))
}

/// Recursively search for a session file by ID
fn find_session_file(id: &str, search_dir: &PathBuf) -> Option<PathBuf> {
    if !search_dir.exists() {
        return None;
    }
    let entries = std::fs::read_dir(search_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_session_file(id, &path) {
                return Some(found);
            }
        } else if path.extension().is_some_and(|ext| ext == "jsonl") {
            // Quick check: does the filename contain the ID?
            if path
                .file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.contains(id))
            {
                // Verify by reading the header
                if let Ok(content) = std::fs::read_to_string(&path)
                    && let Some(first_line) = content.lines().next()
                    && let Ok(header) = serde_json::from_str::<serde_json::Value>(first_line)
                    && header
                        .get("id")
                        .and_then(|v| v.as_str())
                        .is_some_and(|hid| hid.starts_with(id))
                {
                    return Some(path);
                }
            }
        }
    }
    None
}

/// Try to show the TUI session picker.
/// Returns the selected session path, or None if cancelled/picker unavailable.
async fn try_tui_session_picker(_cwd: &PathBuf, project_dir: &PathBuf) -> Option<String> {
    use crate::cli::session_picker::select_session;
    use crate::core::session_manager::{SessionInfo, list_all_sessions, list_sessions_from_dir};
    use futures::future::BoxFuture;

    // Create loaders that list sessions
    let pd = project_dir.clone();
    let global_dir = crate::config::get_sessions_dir();

    let current_sessions_loader: Box<
        dyn Fn() -> BoxFuture<'static, Vec<SessionInfo>> + Send + Sync,
    > = Box::new(move || {
        let dir = pd.clone();
        Box::pin(async move { list_sessions_from_dir(&dir).await })
    });

    let all_sessions_loader: Box<dyn Fn() -> BoxFuture<'static, Vec<SessionInfo>> + Send + Sync> =
        Box::new(move || {
            let gd = global_dir.clone();
            Box::pin(async move { list_all_sessions(&gd).await })
        });

    select_session(current_sessions_loader, all_sessions_loader).await
}
