use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AuthType {
    Once,
    Permanent,
}

/// An authorization entry stored as "ToolName(path_pattern)"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AllowEntry {
    /// The original permission string, e.g. "Bash(cd *)" or "Read(D:\\path\\**)"
    entry: String,
    /// Auth type
    auth_type: AuthType,
    /// When created
    created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AuthFile {
    permissions: PermissionsBlock,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PermissionsBlock {
    allow: Vec<AllowEntry>,
}

pub struct ExternalDirectoryAuth {
    file_path: std::path::PathBuf,
    entries: Mutex<Vec<AllowEntry>>,
}

impl ExternalDirectoryAuth {
    pub fn load(project_dir: &Path) -> Self {
        let file_path = project_dir.join(".pick").join("local.auth.json");
        let entries = if file_path.exists() {
            match std::fs::read_to_string(&file_path) {
                Ok(content) => match serde_json::from_str::<AuthFile>(&content) {
                    Ok(auth_file) => auth_file.permissions.allow,
                    Err(_) => Vec::new(),
                },
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };
        Self {
            file_path,
            entries: Mutex::new(entries),
        }
    }

    /// Check if a tool+path combo is authorized.
    /// Returns the AuthType if found.
    pub fn check(&self, tool: &str, path: &str) -> Option<AuthType> {
        let entries = self.entries.lock().unwrap();
        // Normalize the path for matching, resolving symlinks
        let norm_path = path.replace('\\', "/").trim_end_matches('/').to_string();
        // Resolve symlinks to prevent authorization bypass via symlink in workspace
        let real_path = std::fs::canonicalize(std::path::Path::new(&norm_path))
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or(norm_path.clone());

        for entry in entries.iter() {
            if let Some((entry_tool, entry_pattern)) = parse_permission_string(&entry.entry)
                && entry_tool == tool
                && (match_pattern(entry_pattern, &norm_path)
                    || match_pattern(entry_pattern, &real_path))
            {
                return Some(entry.auth_type);
            }
        }
        None
    }

    /// Add an authorization entry with normalized path.
    pub fn add(&self, tool: &str, path: &str, auth_type: AuthType) {
        let norm_path = path.replace('\\', "/").trim_end_matches('/').to_string();
        let entry_str = format!("{}({})", tool, norm_path);
        let entry = AllowEntry {
            entry: entry_str,
            auth_type,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let mut entries = self.entries.lock().unwrap();
        // Deduplicate: replace existing entry for same tool+path
        entries.retain(|e| {
            if let Some((et, ep)) = parse_permission_string(&e.entry) {
                !(et == tool && ep == norm_path)
            } else {
                true
            }
        });
        entries.push(entry);
    }

    /// Remove a matching entry (for Once consumption).
    pub fn remove(&self, tool: &str, path: &str) {
        let norm_path = path.replace('\\', "/").trim_end_matches('/').to_string();
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|e| {
            if let Some((entry_tool, entry_pattern)) = parse_permission_string(&e.entry) {
                !(entry_tool == tool && match_pattern(entry_pattern, &norm_path))
            } else {
                true
            }
        });
    }

    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.file_path.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create auth dir: {}", e))?;
        }
        let entries = self.entries.lock().unwrap();
        let auth_file = AuthFile {
            permissions: PermissionsBlock {
                allow: entries.clone(),
            },
        };
        let json = serde_json::to_string_pretty(&auth_file)
            .map_err(|e| format!("Failed to serialize auth: {}", e))?;
        std::fs::write(&self.file_path, json)
            .map_err(|e| format!("Failed to write auth file: {}", e))?;
        Ok(())
    }

    pub fn has_permanent(&self, tool: &str, path: &str) -> bool {
        matches!(self.check(tool, path), Some(AuthType::Permanent))
    }

    pub fn consume_once(&self, tool: &str, path: &str) -> bool {
        let norm_path = path.replace('\\', "/").trim_end_matches('/').to_string();
        let real_path = std::fs::canonicalize(std::path::Path::new(&norm_path))
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or(norm_path.clone());

        let mut entries = self.entries.lock().unwrap();
        let pos = entries.iter().position(|e| {
            if let Some((entry_tool, entry_pattern)) = parse_permission_string(&e.entry) {
                entry_tool == tool
                    && e.auth_type == AuthType::Once
                    && (match_pattern(entry_pattern, &norm_path)
                        || match_pattern(entry_pattern, &real_path))
            } else {
                false
            }
        });
        if let Some(idx) = pos {
            entries.remove(idx);
            drop(entries);
            self.save().ok();
            true
        } else {
            false
        }
    }

    /// Revoke all permanent authorizations matching the given tool and path pattern.
    /// Returns the number of entries revoked.
    pub fn revoke(&self, tool: &str, path_pattern: &str) -> usize {
        let norm_pattern = path_pattern
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_string();
        let mut entries = self.entries.lock().unwrap();
        let before = entries.len();
        entries.retain(|e| {
            if let Some((entry_tool, entry_pattern)) = parse_permission_string(&e.entry) {
                !(entry_tool == tool
                    && e.auth_type == AuthType::Permanent
                    && match_pattern(entry_pattern, &norm_pattern))
            } else {
                true
            }
        });
        let removed = before - entries.len();
        if removed > 0 {
            drop(entries);
            self.save().ok();
        }
        removed
    }
}

/// Parse a permission string like "Bash(cd *)" into ("Bash", "cd *")
/// Handles paths containing parentheses by using the first opening paren
/// and the last closing paren.
fn parse_permission_string(s: &str) -> Option<(&str, &str)> {
    let paren_open = s.find('(')?;
    let paren_close = s.rfind(')')?;
    if paren_open < paren_close {
        let tool = &s[..paren_open];
        let pattern = &s[paren_open + 1..paren_close];
        Some((tool, pattern))
    } else {
        None
    }
}

/// Check if path access is authorized. If not in whitelist, prompt user via question callback.
/// Returns Ok(true) = allow, Ok(false) = deny/block.
pub async fn check_authorization(
    tool: &str,
    path: &str,
    pm: &crate::permission::manager::PermissionManager,
    question: Option<&crate::core::state::QuestionFn>,
    tool_event_bus: Option<&Arc<crate::core::hooks::ToolEventBus>>,
) -> Result<bool, String> {
    use crate::core::hooks::{ToolEvent, WaitingKind};
    use crate::core::state::{QuestionOption, QuestionPrompt};

    if pm.external_auth.has_permanent(tool, path) {
        return Ok(true);
    }
    if pm.external_auth.consume_once(tool, path) {
        return Ok(true);
    }

    if let Some(ask) = question {
        // Publish WaitingForUser event before prompting the user
        if let Some(bus) = tool_event_bus {
            bus.publish(&ToolEvent::WaitingForUser {
                tool_name: tool.to_string(),
                tool_call_id: String::new(),
                input: serde_json::json!({"path": path}),
                kind: WaitingKind::Permission {
                    permission: format!("external_dir_{}", tool),
                },
                summary: format!("Tool '{}' requires access to '{}'", tool, path),
            })
            .await;
        }
        let answers = ask(vec![QuestionPrompt {
            header: "External Directory Access".into(),
            question: format!(
                "Allow access to '{}'?\nPath is outside the current workspace",
                path
            ),
            multiple: false,
            options: vec![
                QuestionOption {
                    label: "Allow Once".into(),
                    description: "Allow this one time only".into(),
                },
                QuestionOption {
                    label: "Allow Always".into(),
                    description: "Permanently allow access to this directory".into(),
                },
                QuestionOption {
                    label: "Deny".into(),
                    description: "Reject this request".into(),
                },
            ],
        }])
        .await
        .map_err(|e| format!("Question prompt failed: {}", e))?;

        let choice = answers.first().and_then(|a| a.first()).map(|s| s.as_str());
        match choice {
            Some("Allow Once") => Ok(true),
            Some("Allow Always") => {
                pm.external_auth.add(tool, path, AuthType::Permanent);
                pm.external_auth.save().ok();
                Ok(true)
            }
            _ => Ok(false),
        }
    } else {
        Ok(false)
    }
}

/// Match a path against a pattern.
/// Pattern supports:
///   - `**` matches any number of path components (must have a prefix, e.g. `dir/**`)
///   - `*` matches any single path component (must have a prefix, e.g. `dir/*`)
///   - exact match otherwise
/// Note: bare `*` or `**` patterns are NOT supported — they would match all paths
/// and bypass authorization.
fn match_pattern(pattern: &str, path: &str) -> bool {
    let pat_norm = pattern.replace('\\', "/").trim_end_matches('/').to_string();
    let path_norm = path.trim_end_matches('/').to_string();

    if pat_norm == path_norm {
        return true;
    }

    // Handle ** glob (matches any depth)
    if let Some(prefix) = pat_norm.strip_suffix("/**") {
        let prefix = prefix.trim_end_matches('/');
        return path_norm == prefix || path_norm.starts_with(&format!("{}/", prefix));
    }

    // Handle * glob (matches single component)
    if let Some(prefix) = pat_norm.strip_suffix("/*") {
        let prefix = prefix.trim_end_matches('/');
        if let Some(last_slash) = path_norm.rfind('/') {
            return path_norm[..last_slash] == *prefix;
        }
        return false;
    }

    false
}
