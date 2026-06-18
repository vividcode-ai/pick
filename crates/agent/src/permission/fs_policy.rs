use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AccessMode {
    Read,
    Write,
    Deny,
}

#[derive(Debug, Clone)]
pub struct WritableRoot {
    pub path: PathBuf,
    pub mode: AccessMode,
}

#[derive(Debug, Clone)]
pub struct FileSystemPolicy {
    pub writable_roots: Vec<WritableRoot>,
    pub readable_roots: Vec<PathBuf>,
    pub unreadable_paths: Vec<String>,
    pub protected_paths: Vec<String>,
    pub allow_absolute_paths: bool,
    pub allow_relative_paths: bool,
    pub default_access_mode: AccessMode,
}

/// Centralized list of protected path patterns used across all profiles.
/// Any path matching these patterns is denied read AND write access.
pub fn default_protected_paths() -> Vec<String> {
    vec![
        ".git/**".to_string(),
        ".pick/sessions/**".to_string(),
        ".pick/auth/**".to_string(),
        ".pick/local.auth.json".to_string(),
        ".agents/**".to_string(),
        "node_modules/**".to_string(),
    ]
}

impl Default for FileSystemPolicy {
    fn default() -> Self {
        Self {
            writable_roots: Vec::new(),
            readable_roots: Vec::new(),
            unreadable_paths: Vec::new(),
            protected_paths: default_protected_paths(),
            allow_absolute_paths: false,
            allow_relative_paths: true,
            default_access_mode: AccessMode::Deny,
        }
    }
}

impl FileSystemPolicy {
    pub fn new_workspace_default(workspace_root: &Path) -> Self {
        Self {
            writable_roots: vec![WritableRoot {
                path: workspace_root.to_path_buf(),
                mode: AccessMode::Write,
            }],
            readable_roots: vec![],
            unreadable_paths: vec![],
            protected_paths: default_protected_paths(),
            allow_absolute_paths: false,
            allow_relative_paths: true,
            default_access_mode: AccessMode::Deny,
        }
    }

    pub fn new_readonly(workspace_root: &Path) -> Self {
        Self {
            writable_roots: vec![WritableRoot {
                path: workspace_root.to_path_buf(),
                mode: AccessMode::Read,
            }],
            readable_roots: vec![],
            unreadable_paths: vec![],
            protected_paths: default_protected_paths(),
            allow_absolute_paths: false,
            allow_relative_paths: true,
            default_access_mode: AccessMode::Deny,
        }
    }

    pub fn new_full_access() -> Self {
        Self {
            writable_roots: Vec::new(),
            readable_roots: Vec::new(),
            unreadable_paths: Vec::new(),
            protected_paths: Vec::new(),
            allow_absolute_paths: true,
            allow_relative_paths: true,
            default_access_mode: AccessMode::Write,
        }
    }

    pub fn can_read(&self, path: &Path, cwd: &Path) -> Result<(), String> {
        let resolved = self.resolve_path(path, cwd)?;

        if self.is_protected(&resolved) {
            return Err(format!(
                "Read access denied: '{}' is a protected path",
                resolved.display()
            ));
        }

        // Check default access mode first (affects full_access profile with empty roots)
        // Write implies read access, so both modes allow reads.
        if self.default_access_mode == AccessMode::Read
            || self.default_access_mode == AccessMode::Write
        {
            return Ok(());
        }

        // Check writable roots for read access
        let allowed = self.writable_roots.iter().any(|root| {
            if root.mode == AccessMode::Deny {
                return false;
            }
            resolved.starts_with(&root.path)
        });

        if allowed {
            Ok(())
        } else if self.writable_roots.is_empty() && self.readable_roots.is_empty() {
            // Empty roots + default Deny = allow nothing
            Err(format!(
                "Read access denied: '{}' — no access roots configured",
                resolved.display()
            ))
        } else {
            // Check readable roots
            let readable = self
                .readable_roots
                .iter()
                .any(|root| resolved.starts_with(root));
            if readable {
                Ok(())
            } else {
                Err(format!(
                    "Read access denied: '{}' is not within any readable or writable root",
                    resolved.display()
                ))
            }
        }
    }

    pub fn can_write(&self, path: &Path, cwd: &Path) -> Result<(), String> {
        let resolved = self.resolve_path(path, cwd)?;

        if self.is_protected(&resolved) {
            return Err(format!(
                "Write access denied: '{}' is a protected path",
                resolved.display()
            ));
        }

        // Check default access mode first
        if self.default_access_mode == AccessMode::Write {
            return Ok(());
        }

        if self.writable_roots.is_empty() {
            return Err(format!(
                "Write access denied: '{}' — no writable roots configured",
                resolved.display()
            ));
        }

        let allowed = self
            .writable_roots
            .iter()
            .any(|root| root.mode == AccessMode::Write && resolved.starts_with(&root.path));

        if allowed {
            Ok(())
        } else {
            Err(format!(
                "Write access denied: '{}' is not within any writable root with write permission",
                resolved.display()
            ))
        }
    }

    fn has_root_with_mode(&self, mode: AccessMode) -> bool {
        self.writable_roots.iter().any(|r| r.mode == mode)
    }

    pub fn can_execute(&self, path: &Path, cwd: &Path) -> Result<(), String> {
        self.can_read(path, cwd)
    }

    fn resolve_path(&self, path: &Path, cwd: &Path) -> Result<PathBuf, String> {
        let is_absolute = path.is_absolute();

        if is_absolute && !self.allow_absolute_paths {
            return Err(format!(
                "Absolute paths are not allowed: '{}'",
                path.display()
            ));
        }

        if !is_absolute && !self.allow_relative_paths {
            return Err(format!(
                "Relative paths are not allowed: '{}'",
                path.display()
            ));
        }

        let base = if is_absolute {
            path.to_path_buf()
        } else {
            let mut base = cwd.to_path_buf();
            base.push(path);
            base
        };

        let normalized = normalize_path(&base);

        // Resolve symlinks to prevent workspace escape attacks.
        // If the full path doesn't exist yet (e.g. writing a new file),
        // resolve the parent directory and append the file name.
        match dunce::canonicalize(&normalized) {
            Ok(canonical) => Ok(canonical),
            Err(_) => {
                if let Some(parent) = normalized.parent() {
                    if let Ok(canonical_parent) = dunce::canonicalize(parent) {
                        if let Some(file_name) = normalized.file_name() {
                            return Ok(canonical_parent.join(file_name));
                        }
                    }
                }
                Ok(normalized)
            }
        }
    }

    fn is_protected(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().replace('\\', "/");
        for pattern in &self.protected_paths {
            if glob_match(pattern, &path_str) {
                return true;
            }
        }
        false
    }

    pub fn resolve_access(&self, path: &Path, cwd: &Path) -> Result<AccessMode, String> {
        let resolved = self.resolve_path(path, cwd)?;

        // 1. Check unreadable paths (most specific, glob patterns)
        let path_str = resolved.to_string_lossy().replace('\\', "/");
        for pattern in &self.unreadable_paths {
            if glob_match(pattern, &path_str) {
                return Ok(AccessMode::Deny);
            }
        }

        // 2. Check protected paths
        if self.is_protected(&resolved) {
            return Ok(AccessMode::Deny);
        }

        // 3. Check writable roots (prefer most specific match by path length)
        let mut best_mode: Option<AccessMode> = None;
        let mut best_depth: usize = 0;
        for root in &self.writable_roots {
            if resolved.starts_with(&root.path) {
                let depth = root.path.components().count();
                if best_mode.is_none() || depth > best_depth {
                    best_mode = Some(root.mode);
                    best_depth = depth;
                }
            }
        }
        if let Some(mode) = best_mode {
            return Ok(mode);
        }

        // 4. Check readable roots
        for root in &self.readable_roots {
            if resolved.starts_with(root) {
                return Ok(AccessMode::Read);
            }
        }

        // 5. Default
        Ok(self.default_access_mode)
    }

    pub fn allow_absolute_paths(&self) -> bool {
        self.allow_absolute_paths
    }

    /// Check if a path resolves to a protected path (hard deny, not authorizable).
    pub fn is_path_protected(&self, path: &Path, cwd: &Path) -> Result<bool, String> {
        let resolved = self.resolve_path(path, cwd)?;
        Ok(self.is_protected(&resolved))
    }
}

/// Extract tokens from a command string that look like absolute file paths
pub fn extract_absolute_path_args(command: &str) -> Vec<String> {
    command
        .split_whitespace()
        .filter(|token| {
            let cleaned = token.trim_matches(|c: char| c == '"' || c == '\'' || c == '`');
            if cleaned.len() < 2 {
                return false;
            }
            // Skip tokens containing shell expansions — they cannot be reliably checked
            if contains_shell_expansion(cleaned) {
                return false;
            }
            // Windows: D:\path or D:/path
            (cleaned.len() >= 3
                && cleaned.as_bytes()[1] == b':'
                && (cleaned.as_bytes()[2] == b'\\' || cleaned.as_bytes()[2] == b'/'))
            // Unix: /path
            || cleaned.starts_with('/')
            // UNC: \\server\share
            || cleaned.starts_with("\\\\")
        })
        .map(|t| {
            t.trim_matches(|c: char| c == '"' || c == '\'' || c == '`')
                .to_string()
        })
        .collect()
}

/// Check if a command string contains shell expansions that make path analysis unreliable.
pub fn contains_shell_expansion(command: &str) -> bool {
    command.contains('$')
        || command.contains('`')
        || command.contains('~')
        || command.contains('*')
        || command.contains('?')
        || command.contains('[')
        || command.contains(']')
        || command.contains('{')
        || command.contains('}')
}

fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !components.is_empty() && components.last() != Some(&Component::ParentDir) {
                    components.pop();
                } else {
                    components.push(component);
                }
            }
            other => components.push(other),
        }
    }
    components.iter().collect()
}

fn glob_match(pattern: &str, path: &str) -> bool {
    if pattern == path {
        return true;
    }

    if let Some(prefix) = pattern.strip_suffix("/**") {
        let prefix = prefix.trim_end_matches('/');
        let path_trimmed = path.trim_end_matches('/');
        // Match the directory itself or any path under it (direct or nested)
        path_trimmed == prefix
            || path_trimmed.starts_with(&format!("{}/", prefix))
            // Handle subdirectory matches: pattern "dir/**" matches "/repo/dir/file"
            || path_trimmed.contains(&format!("/{}/", prefix))
            // Handle trailing case: pattern "dir/**" matches "/repo/dir"
            || path_trimmed.ends_with(&format!("/{}", prefix))
    } else if let Some(prefix) = pattern.strip_suffix("/*") {
        let prefix = prefix.trim_end_matches('/');
        if let Some(last_slash) = path.rfind('/') {
            path[..last_slash].trim_end_matches('/') == prefix
        } else {
            false
        }
    } else if let Some(suffix) = pattern.strip_prefix("**/") {
        path.ends_with(suffix) || path.contains(&format!("/{}", suffix))
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glob_match_test(pattern: &str, path: &str) -> bool {
        glob_match(pattern, path)
    }

    #[test]
    fn test_glob_match_exact() {
        assert!(glob_match_test(".git/config", ".git/config"));
        assert!(!glob_match_test(".git/config", ".git/other"));
    }

    #[test]
    fn test_glob_match_doublestar() {
        assert!(glob_match_test(".git/**", ".git/config"));
        assert!(glob_match_test(".git/**", ".git/objects/abc123"));
        assert!(glob_match_test(".git/**", ".git"));
        assert!(!glob_match_test(".git/**", ".gitt/config"));
    }

    #[test]
    fn test_glob_match_singlestar() {
        assert!(glob_match_test("src/*", "src/main.rs"));
        assert!(!glob_match_test("src/*", "src/sub/lib.rs"));
    }

    #[test]
    fn test_glob_match_prefix_star() {
        assert!(glob_match_test("**/.env", "/project/.env"));
        assert!(glob_match_test("**/.env", ".env"));
    }

    #[test]
    fn test_protected_path_patterns() {
        let policy = FileSystemPolicy::default();
        assert!(policy.is_protected(Path::new("/repo/.git/config")));
        assert!(policy.is_protected(Path::new("/repo/.pick/sessions/abc.jsonl")));
        assert!(!policy.is_protected(Path::new("/repo/src/main.rs")));
        assert!(policy.is_protected(Path::new("C:/repo/.git/config")));
        assert!(policy.is_protected(Path::new("C:/repo/.pick/auth/key.json")));
    }

    #[test]
    fn test_can_write_within_root() {
        let tmp = std::env::temp_dir().join("Pick-fs-test-write");
        let _ = std::fs::create_dir_all(&tmp);

        let canonical_tmp = dunce::canonicalize(&tmp).unwrap();
        let policy = FileSystemPolicy::new_workspace_default(&canonical_tmp);
        let test_file = tmp.join("test.txt");
        std::fs::write(&test_file, "hello").ok();

        let result = policy.can_write(Path::new("test.txt"), &canonical_tmp);
        assert!(result.is_ok(), "can_write failed: {:?}", result);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_cannot_write_outside_root() {
        let tmp = std::env::temp_dir().join("Pick-fs-test-outside");
        let _ = std::fs::create_dir_all(&tmp);

        let outside = std::env::temp_dir().join("Pick-fs-outside");
        let _ = std::fs::create_dir_all(&outside);

        let canonical_tmp = dunce::canonicalize(&tmp).unwrap();
        let policy = FileSystemPolicy::new_workspace_default(&canonical_tmp);
        let result = policy.can_write(Path::new("../Pick-fs-outside/evil.txt"), &canonical_tmp);
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&tmp);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn test_cannot_write_protected_git() {
        let tmp = std::env::temp_dir().join("Pick-fs-test-protected");
        let git_dir = tmp.join(".git");
        let _ = std::fs::create_dir_all(&git_dir);

        let canonical_tmp = dunce::canonicalize(&tmp).unwrap();
        let policy = FileSystemPolicy::new_workspace_default(&canonical_tmp);
        let result = policy.can_write(Path::new(".git/config"), &canonical_tmp);
        assert!(result.is_err(), "Expected error but got: {:?}", result);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_readonly_policy() {
        let tmp = std::env::temp_dir().join("Pick-fs-test-readonly");
        let _ = std::fs::create_dir_all(&tmp);
        let test_file = tmp.join("readme.md");
        std::fs::write(&test_file, "content").ok();

        let canonical_tmp = dunce::canonicalize(&tmp).unwrap();
        let policy = FileSystemPolicy::new_readonly(&canonical_tmp);
        let read_ok = policy.can_read(Path::new("readme.md"), &canonical_tmp);
        assert!(read_ok.is_ok(), "can_read failed: {:?}", read_ok);

        let write_err = policy.can_write(Path::new("readme.md"), &canonical_tmp);
        assert!(write_err.is_err());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_full_access_allows_any_path() {
        let policy = FileSystemPolicy::new_full_access();
        let tmp = std::env::temp_dir();
        let writable_path = tmp.join("Pick-fs-test-any.txt");
        let result = policy.can_write(&writable_path, &tmp);
        assert!(result.is_ok(), "can_write failed: {:?}", result);

        let abs = PathBuf::from(if cfg!(windows) {
            "C:/test.txt"
        } else {
            "/tmp/test.txt"
        });
        let result2 = policy.can_write(&abs, &tmp);
        assert!(
            result2.is_ok(),
            "absolute path should work with full access: {:?}",
            result2
        );
    }

    #[test]
    fn test_protected_path_read_denied() {
        let tmp = std::env::temp_dir().join("Pick-fs-test-protected-read");
        let git_dir = tmp.join(".git");
        let _ = std::fs::create_dir_all(&git_dir);

        let canonical_tmp = dunce::canonicalize(&tmp).unwrap();
        let policy = FileSystemPolicy::new_readonly(&canonical_tmp);
        let result = policy.can_read(Path::new(".git/config"), &canonical_tmp);
        assert!(result.is_err(), "Expected error but got: {:?}", result);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_absolute_path_denied_by_default() {
        let tmp = std::env::temp_dir().join("Pick-fs-test-abs");
        let _ = std::fs::create_dir_all(&tmp);

        let canonical_tmp = dunce::canonicalize(&tmp).unwrap();
        let policy = FileSystemPolicy::new_workspace_default(&canonical_tmp);

        let result = policy.can_write(Path::new("test.txt"), &canonical_tmp);
        assert!(result.is_ok(), "relative write should work: {:?}", result);

        let abs_path = PathBuf::from(if cfg!(windows) {
            "C:/Windows/test.txt"
        } else {
            "/etc/test.txt"
        });
        let result2 = policy.can_write(&abs_path, &canonical_tmp);
        assert!(
            result2.is_err(),
            "absolute path should be denied with workspace default: {:?}",
            result2
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
