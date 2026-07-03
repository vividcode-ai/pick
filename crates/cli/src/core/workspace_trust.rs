use std::io::Write;
use std::path::Path;

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::queue;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use serde::{Deserialize, Serialize};

use crate::config::get_workspaces_path;

#[derive(Debug, Serialize, Deserialize)]
struct WorkspaceList {
    workspaces: Vec<String>,
}

fn load() -> WorkspaceList {
    let path = get_workspaces_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or(WorkspaceList {
            workspaces: Vec::new(),
        }),
        Err(_) => WorkspaceList {
            workspaces: Vec::new(),
        },
    }
}

fn save(list: &WorkspaceList) {
    let path = get_workspaces_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(list) {
        let _ = std::fs::write(&path, &content);
    }
}

/// Normalize a path for comparison (canonicalize if possible, otherwise just
/// convert to an absolute path with forward-slash normalization).
fn normalize_path(path: &Path) -> String {
    if let Ok(canon) = dunce::canonicalize(path) {
        return canon.to_string_lossy().replace('\\', "/");
    }
    // Fallback: try to make it absolute
    let path = if path.is_relative() {
        if let Ok(cwd) = std::env::current_dir() {
            cwd.join(path)
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    };
    path.to_string_lossy().replace('\\', "/")
}

/// Check whether `cwd` is in the trusted workspace list.
pub fn is_workspace_trusted(cwd: &Path) -> bool {
    let list = load();
    let normalized = normalize_path(cwd);

    // Case-insensitive comparison on Windows
    #[cfg(windows)]
    {
        let normalized_lower = normalized.to_lowercase();
        list.workspaces
            .iter()
            .any(|w| w.to_lowercase() == normalized_lower)
    }
    #[cfg(not(windows))]
    {
        list.workspaces.contains(&normalized)
    }
}

/// Show a workspace trust confirmation dialog in the alternate screen buffer.
/// Returns `true` if the user trusts the folder, `false` otherwise.
pub fn confirm_and_trust_workspace(cwd: &Path) -> bool {
    if enable_raw_mode().is_err() {
        return true;
    }

    let mut stdout = std::io::stdout();
    let _ = queue!(stdout, EnterAlternateScreen);
    let _ = stdout.flush();

    let width = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
    let cwd_display = cwd.to_string_lossy();
    let sep = "\u{2500}".repeat(width as usize);
    let dim = "\x1b[2m";
    let reset = "\x1b[0m";

    let mut selected: usize = 0;
    let options = ["Yes, trust this workspace", "No, exit"];

    let leave_alternate = |stdout: &mut std::io::Stdout| {
        let _ = queue!(stdout, LeaveAlternateScreen);
        let _ = stdout.flush();
        let _ = disable_raw_mode();
    };

    loop {
        let mut lines: Vec<String> = Vec::new();

        lines.push(format!("{dim}{sep}{reset}"));
        lines.push(String::new());
        lines.push(format!("  \x1b[1mWorkspace:{reset} {cwd_display}"));
        lines.push(String::new());
        lines.push("  Pick needs permission to read, write, and execute".to_string());
        lines.push("  files and programs in this directory.".to_string());
        lines.push(String::new());

        for (i, opt) in options.iter().enumerate() {
            if i == selected {
                lines.push(format!("  {dim}\u{276f}{reset} {opt}"));
            } else {
                lines.push(format!("    {opt}"));
            }
        }

        lines.push(String::new());
        lines.push(format!("  {dim}Enter/\u{2191}\u{2193} select  \u{00b7} 1/2 shortcut  \u{00b7} Esc cancel{reset}"));
        lines.push(format!("{dim}{sep}{reset}"));

        for (row, line) in lines.iter().enumerate() {
            let _ = queue!(
                stdout,
                crossterm::cursor::MoveTo(0, row as u16),
                crossterm::style::Print(line)
            );
        }
        let _ = stdout.flush();

        match crossterm::event::read() {
            Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    selected = (selected + 1).min(options.len() - 1);
                }
                KeyCode::Char('1') => selected = 0,
                KeyCode::Char('2') => selected = 1,
                KeyCode::Enter => {
                    let trusted = selected == 0;
                    if trusted {
                        add_workspace(cwd);
                    }
                    leave_alternate(&mut stdout);
                    return trusted;
                }
                KeyCode::Esc => {
                    leave_alternate(&mut stdout);
                    return false;
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    leave_alternate(&mut stdout);
                    std::process::exit(130);
                }
                _ => {}
            },
            Ok(Event::Key(_)) => {}
            _ => {}
        }
    }
}

/// Add a workspace path to the trusted list (idempotent).
fn add_workspace(cwd: &Path) {
    let mut list = load();
    let normalized = normalize_path(cwd);

    let exists = {
        #[cfg(windows)]
        {
            let normalized_lower = normalized.to_lowercase();
            list.workspaces
                .iter()
                .any(|w| w.to_lowercase() == normalized_lower)
        }
        #[cfg(not(windows))]
        {
            list.workspaces.contains(&normalized)
        }
    };

    if !exists {
        list.workspaces.push(normalized);
        save(&list);
    }
}
