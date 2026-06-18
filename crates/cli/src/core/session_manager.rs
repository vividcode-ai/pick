//! Session listing utilities for the new format (pick_agent::session)
//! Reads JSONL files written by pick_agent::session::SessionManager.
//! No backward compatibility with old format.

use std::path::{Path, PathBuf};

use chrono::Utc;

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub path: String,
    pub id: String,
    pub cwd: String,
    pub name: Option<String>,
    pub parent_session_path: Option<String>,
    pub created: chrono::DateTime<Utc>,
    pub modified: chrono::DateTime<Utc>,
    pub message_count: usize,
    pub first_message: String,
    pub all_messages_text: String,
    pub git_branch: Option<String>,
    pub model: String,
}

/// Session info for display in the session tree/selector
#[derive(Debug, Clone)]
pub struct SessionDisplayInfo {
    pub path: String,
    pub name: Option<String>,
    pub first_message: Option<String>,
    pub message_count: u32,
    pub modified: String,
    pub cwd: Option<String>,
    pub parent_session_path: Option<String>,
    pub is_current: bool,
}

/// Flat session node for tree display
#[derive(Debug, Clone)]
pub struct FlatSessionNode {
    pub session: SessionDisplayInfo,
    pub depth: usize,
    pub is_last: bool,
    pub ancestor_continues: Vec<bool>,
}

// ============================================================================
// Session directory helpers
// ============================================================================

/// Compute the default session directory for a cwd
pub fn get_default_session_dir(cwd: &str, agent_dir: &str) -> PathBuf {
    let safe_path = format!("--{}--", cwd.trim_start_matches(|c: char| c == '/' || c == '\\')
        .replace(|c: char| c == '/' || c == ':' || c == '\\', "-"));
    let session_dir = Path::new(agent_dir).join("sessions").join(&safe_path);
    std::fs::create_dir_all(&session_dir).ok();
    session_dir
}

// ============================================================================
// Session listing (reads new-format JSONL files)
// ============================================================================

pub async fn list_sessions_from_dir(dir: &Path) -> Vec<SessionInfo> {
    let mut sessions = Vec::new();
    if !dir.exists() {
        return sessions;
    }
    let mut files = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".jsonl"))
            .map(|e| e.path())
            .collect::<Vec<_>>(),
        Err(_) => return sessions,
    };
    files.sort();
    for file in &files {
        if let Some(info) = build_session_info(file).await {
            sessions.push(info);
        }
    }
    sessions.sort_by(|a, b| b.modified.cmp(&a.modified));
    sessions
}

pub async fn list_all_sessions(sessions_dir: &Path) -> Vec<SessionInfo> {
    if !sessions_dir.exists() {
        return Vec::new();
    }
    let mut all = Vec::new();
    let entries = match std::fs::read_dir(sessions_dir) {
        Ok(e) => e.flatten().collect::<Vec<_>>(),
        Err(_) => return Vec::new(),
    };
    for entry in &entries {
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            let dir = entry.path();
            let sessions = list_sessions_from_dir(&dir).await;
            all.extend(sessions);
        }
    }
    all.sort_by(|a, b| b.modified.cmp(&a.modified));
    all
}

/// Build SessionInfo by reading a new-format JSONL session file.
/// Reads entries as generic serde_json::Value to be format-agnostic.
async fn build_session_info(file_path: &Path) -> Option<SessionInfo> {
    let content = tokio::fs::read_to_string(file_path).await.ok()?;
    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return None;
    }

    // Parse header (first line)
    let header: serde_json::Value = serde_json::from_str(lines[0]).ok()?;
    if header.get("id").and_then(|v| v.as_str()).is_none() {
        return None;
    }

    let id = header.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let cwd = header.get("cwd").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let created_ts = header.get("created_at").and_then(|v| v.as_i64()).unwrap_or(0);
    let updated_ts = header.get("updated_at").and_then(|v| v.as_i64()).unwrap_or(0);
    let model_from_header = header.get("model").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let provider_from_header = header.get("provider").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let created = chrono::DateTime::from_timestamp_millis(created_ts).unwrap_or_else(Utc::now);
    let modified = chrono::DateTime::from_timestamp_millis(updated_ts).unwrap_or_else(Utc::now);

    let mut message_count = 0;
    let mut first_message = String::new();
    let mut all_messages: Vec<String> = Vec::new();
    let mut name: Option<String> = None;
    let mut model: String = if !model_from_header.is_empty() {
        if !provider_from_header.is_empty() {
            format!("{}/{}", provider_from_header, model_from_header)
        } else {
            model_from_header
        }
    } else {
        String::new()
    };

    for &line in &lines[1..] {
        let entry: serde_json::Value = serde_json::from_str(line).ok()?;
        let type_ = entry.get("type").and_then(|v| v.as_str());

        if type_ == Some("session_info") {
            name = entry.get("name").and_then(|v| v.as_str()).map(|s| s.trim().to_string());
            if name.as_deref() == Some("") {
                name = None;
            }
        }

        if type_ == Some("model_change") {
            if let (Some(p), Some(m)) = (
                entry.get("from").and_then(|v| v.as_str()),
                entry.get("to").and_then(|v| v.as_str()),
            ) {
                model = format!("{}/{}", p, m);
            }
        }

        if type_ != Some("message") {
            continue;
        }

        message_count += 1;
        let role = entry.get("role").and_then(|v| v.as_str()).unwrap_or("");
        if role != "user" && role != "assistant" {
            continue;
        }

        if model.is_empty() && role == "assistant" {
            if let (Some(p), Some(m)) = (
                entry.get("provider").and_then(|v| v.as_str()),
                entry.get("model").and_then(|v| v.as_str()),
            ) {
                model = format!("{}/{}", p, m);
            }
        }

        // Extract text content from content blocks
        if let Some(content) = entry.get("content") {
            if let Some(blocks) = content.as_array() {
                for block in blocks {
                    if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                        if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                            if !text.is_empty() {
                                all_messages.push(text.to_string());
                                if first_message.is_empty() && role == "user" {
                                    first_message = text.to_string();
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let git_branch = detect_git_branch_for_cwd(&cwd);
    if model.is_empty() {
        model = "unknown".to_string();
    }

    Some(SessionInfo {
        path: file_path.to_string_lossy().to_string(),
        id,
        cwd,
        name,
        parent_session_path: None,
        created,
        modified,
        message_count,
        first_message: if first_message.is_empty() { "(no messages)".to_string() } else { first_message },
        all_messages_text: all_messages.join(" "),
        git_branch,
        model,
    })
}

/// Detect git branch for a given working directory
fn detect_git_branch_for_cwd(cwd: &str) -> Option<String> {
    let cwd_path = std::path::Path::new(cwd);
    let mut dir = Some(cwd_path);
    while let Some(d) = dir {
        let git_dir = d.join(".git");
        if git_dir.is_dir() {
            let head_path = git_dir.join("HEAD");
            if let Ok(head) = std::fs::read_to_string(head_path) {
                let head = head.trim();
                if let Some(ref_name) = head.strip_prefix("ref: refs/heads/") {
                    return Some(ref_name.to_string());
                } else if !head.is_empty() {
                    return Some("detached".to_string());
                }
            }
            return None;
        }
        dir = d.parent();
    }
    None
}

// ============================================================================
// Session tree display utilities
// ============================================================================

fn shorten_path(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("/home/").or_else(|| path.strip_prefix("/Users/")) {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() > 1 {
            return format!("~{}/{}", parts[0], parts[1..].join("/"));
        }
        return format!("~{}", parts[0]);
    }
    if let Some(rest) = path.strip_prefix("C:\\Users\\").or_else(|| path.strip_prefix("c:\\Users\\")) {
        let parts: Vec<&str> = rest.split('\\').collect();
        if parts.len() > 1 {
            return format!("~{}/{}", parts[0], parts[1..].join("/"));
        }
        return format!("~{}", parts[0]);
    }
    path.to_string()
}

fn format_session_date(iso_timestamp: &str) -> String {
    if iso_timestamp.is_empty() { "now".to_string() } else { iso_timestamp.to_string() }
}

/// Build session tree from flat session list, returning flat nodes with tree structure.
pub fn build_session_tree(sessions: &[SessionDisplayInfo]) -> Vec<FlatSessionNode> {
    let mut children: Vec<Vec<usize>> = vec![vec![]; sessions.len()];
    let mut roots = Vec::new();

    for (i, session) in sessions.iter().enumerate() {
        let parent_path = session.parent_session_path.as_deref();
        let parent_idx = parent_path.and_then(|pp| {
            sessions.iter().position(|s| s.path == pp)
        });
        if let Some(idx) = parent_idx {
            children[idx].push(i);
        } else {
            roots.push(i);
        }
    }

    for child_group in &mut children {
        child_group.sort_by(|&a, &b| {
            sessions[b].modified.as_str().cmp(sessions[a].modified.as_str())
        });
    }
    roots.sort_by(|&a, &b| {
        sessions[b].modified.as_str().cmp(sessions[a].modified.as_str())
    });

    let mut result = Vec::new();
    fn walk(
        idx: usize,
        sessions: &[SessionDisplayInfo],
        children: &[Vec<usize>],
        depth: usize,
        ancestor_continues: &[bool],
        is_last: bool,
        result: &mut Vec<FlatSessionNode>,
    ) {
        result.push(FlatSessionNode {
            session: sessions[idx].clone(),
            depth,
            is_last,
            ancestor_continues: ancestor_continues.to_vec(),
        });
        let child_count = children[idx].len();
        for (ci, &child) in children[idx].iter().enumerate() {
            let child_is_last = ci == child_count - 1;
            let continues = if depth > 0 { !is_last } else { false };
            let mut ancestor = ancestor_continues.to_vec();
            if depth > 0 {
                ancestor.push(continues);
            }
            walk(child, sessions, children, depth + 1, &ancestor, child_is_last, result);
        }
    }

    for &root in &roots {
        walk(root, &sessions, &children, 0, &[], true, &mut result);
    }
    result
}
