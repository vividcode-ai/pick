//! TUI session selector for --resume/session flag

//!
//! Enhanced with richer display: first message preview, short ID, cwd,
//! time ago, git branch, model. Supports Tab toggle, preview (Ctrl+E),
//! and delete (Delete key).

use crate::core::session_manager::SessionInfo;
use crate::utils::tui_wrapper::{run_extended_selector, show_session_preview, read_single_key, ExtendedSelectResult};

/// Session loader function type
pub type SessionsLoader = Box<dyn Fn() -> futures::future::BoxFuture<'static, Vec<SessionInfo>> + Send + Sync>;

/// Show TUI session selector and return selected session path or None if cancelled.
/// Supports scope toggle (Tab), preview (Ctrl+E), delete (Delete).
pub async fn select_session(
    current_sessions_loader: SessionsLoader,
    all_sessions_loader: SessionsLoader,
) -> Option<String> {
    let mut use_current_scope = true;

    loop {
        let sessions = if use_current_scope {
            let s = current_sessions_loader().await;
            if s.is_empty() { all_sessions_loader().await } else { s }
        } else {
            all_sessions_loader().await
        };

        if sessions.is_empty() {
            if !use_current_scope {
                return None;
            }
            // Try all scope if current is empty
            let s = all_sessions_loader().await;
            if s.is_empty() {
                return None;
            }
        }

        let scope_name = if use_current_scope { "current" } else { "all" };

        let result = run_extended_selector(
            "Select Session",
            &sessions,
            render_session_item,
            scope_name,
        );

        match result {
            ExtendedSelectResult::Selected(idx) => {
                return sessions.get(idx).map(|s| s.path.clone());
            }
            ExtendedSelectResult::Cancelled => {
                return None;
            }
            ExtendedSelectResult::ToggleScope => {
                use_current_scope = !use_current_scope;
                continue;
            }
            ExtendedSelectResult::Preview(idx) => {
                if let Some(session) = sessions.get(idx) {
                    let first_msg_preview = if session.first_message.len() > 60 {
                        format!("{}...", &session.first_message[..57])
                    } else {
                        session.first_message.clone()
                    };
                    show_session_preview(
                        &first_msg_preview,
                        &session.all_messages_text,
                        session.message_count,
                        &session.id,
                    );
                }
                continue;
            }
            ExtendedSelectResult::Delete(idx) => {
                if let Some(session) = sessions.get(idx) {
                    let path = &session.path;
                    // Confirm deletion
                    if confirm_deletion(&session) {
                        let _ = std::fs::remove_file(path);
                    }
                }
                continue;
            }
        }
    }
}

/// Render a single session item as multiple lines for the extended selector.
fn render_session_item(idx: usize, session: &SessionInfo) -> Vec<String> {
    let mut lines = Vec::new();

    // Separator between sessions
    if idx > 0 {
        lines.push(String::new());
    }

    // Line 1: serial number + session title (name or first message)
    let title = session.name.as_deref()
        .filter(|n| !n.is_empty())
        .map(|n| n.to_string())
        .unwrap_or_else(|| {
            if session.first_message.len() > 80 {
                format!("{}...", &session.first_message[..77])
            } else {
                session.first_message.clone()
            }
        });
    lines.push(format!("{:>3}. {}", idx + 1, title));

    // Line 2: metadata
    let mut meta_parts: Vec<String> = Vec::new();

    // Short ID
    let short_id = if session.id.len() > 8 {
        format!("{}", &session.id[..8])
    } else {
        session.id.clone()
    };
    meta_parts.push(format!("\x1b[2m{}\x1b[0m", short_id));

    // CWD (shortened)
    let cwd_short = shorten_path(&session.cwd);
    meta_parts.push(cwd_short);

    // Git branch
    if let Some(ref branch) = session.git_branch {
        meta_parts.push(format!("\x1b[34m{}\x1b[0m", branch));
    }

    // Model
    if session.model != "unknown" {
        meta_parts.push(format!("\x1b[2m{}\x1b[0m", session.model));
    }

    // Time ago
    let time_ago = format_time_ago(session.modified);
    meta_parts.push(format!("\x1b[2m{}\x1b[0m", time_ago));

    let meta_line = meta_parts.join("  \x1b[2m\u{2022}\x1b[0m  ");
    if !meta_line.is_empty() {
        lines.push(format!("   {}", meta_line));
    }

    lines
}

/// Time ago formatter
fn format_time_ago(dt: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let dur = now.signed_duration_since(dt);

    if dur.num_seconds() < 60 {
        "just now".to_string()
    } else if dur.num_minutes() < 60 {
        format!("{}m ago", dur.num_minutes())
    } else if dur.num_hours() < 24 {
        format!("{}h ago", dur.num_hours())
    } else if dur.num_days() < 7 {
        format!("{}d ago", dur.num_days())
    } else if dur.num_weeks() < 52 {
        format!("{}w ago", dur.num_weeks())
    } else {
        format!("{}y ago", dur.num_weeks() / 52)
    }
}

/// Shorten path for display
fn shorten_path(path: &str) -> String {
    // Home dir shortening
    if let Some(rest) = path.strip_prefix("/home/").or_else(|| path.strip_prefix("/Users/")) {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() > 1 {
            return format!("~/{}/.../{}", parts[0], parts.last().unwrap_or(&""));
        }
        return format!("~{}", parts[0]);
    }
    if let Some(rest) = path.strip_prefix("C:\\Users\\").or_else(|| path.strip_prefix("c:\\Users\\")) {
        let parts: Vec<&str> = rest.split('\\').collect();
        if parts.len() > 1 {
            return format!("~/{}/.../{}", parts[0], parts.last().unwrap_or(&""));
        }
        return format!("~{}", parts[0]);
    }
    // Just use last 2 path components
    let parts: Vec<&str> = path.split(&['/', '\\'][..]).filter(|s| !s.is_empty()).collect();
    if parts.len() > 2 {
        format!(".../{}", parts[parts.len()-2..].join("/"))
    } else if parts.len() > 1 {
        parts[parts.len()-2..].join("/")
    } else {
        path.to_string()
    }
}

/// Show deletion confirmation and return true if user confirms
fn confirm_deletion(session: &SessionInfo) -> bool {
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
    use std::io::Write;

    let _ = enable_raw_mode();
    let mut stdout = std::io::stdout();

    let short_id = if session.id.len() > 8 {
        &session.id[..8]
    } else {
        &session.id
    };

    let msg = format!(
        "\rDelete session {} ({} msgs)? [y/N] ",
        short_id, session.message_count
    );
    let _ = stdout.write_all(msg.as_bytes());
    let _ = stdout.flush();

    // Read a single key
    let result = loop {
        match read_single_key() {
            Some('y') | Some('Y') => break true,
            Some(_) => break false,
            None => continue,
        }
    };

    let _ = writeln!(stdout);
    let _ = stdout.flush();
    let _ = disable_raw_mode();

    if result {
        eprintln!("Deleted session {}", short_id);
    }
    result
}
