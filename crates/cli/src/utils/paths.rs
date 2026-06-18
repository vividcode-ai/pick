//! Path utilities

use std::path::{Path, PathBuf};

/// Options for path normalization
pub struct PathInputOptions {
    pub trim: bool,
    pub expand_tilde: bool,
    pub home_dir: Option<String>,
    pub strip_at_prefix: bool,
}

impl Default for PathInputOptions {
    fn default() -> Self {
        Self {
            trim: false,
            expand_tilde: true,
            home_dir: None,
            strip_at_prefix: false,
        }
    }
}

/// Resolve a path to its canonical (real) form, following symlinks.
pub fn canonicalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Check if a path is local (not a package source or URL)
pub fn is_local_path(value: &str) -> bool {
    let trimmed = value.trim();
    !(trimmed.starts_with("npm:")
        || trimmed.starts_with("git:")
        || trimmed.starts_with("github:")
        || trimmed.starts_with("http:")
        || trimmed.starts_with("https:")
        || trimmed.starts_with("ssh:"))
}

/// Normalize a path string
pub fn normalize_path(input: &str, options: &PathInputOptions) -> String {
    let mut normalized = if options.trim {
        input.trim().to_string()
    } else {
        input.to_string()
    };

    if options.strip_at_prefix && normalized.starts_with('@') {
        normalized = normalized[1..].to_string();
    }

    if options.expand_tilde {
        let os_home = dirs::home_dir();
        let home = options.home_dir.as_deref()
            .or_else(|| os_home.as_ref().and_then(|h| h.to_str()))
            .unwrap_or("~");
        if normalized == "~" {
            return home.to_string();
        }
        if normalized.starts_with("~/") {
            return Path::new(home).join(&normalized[2..]).to_string_lossy().to_string();
        }
    }

    normalized
}

/// Resolve a path relative to a base directory
pub fn resolve_path(input: &str, base_dir: &Path) -> PathBuf {
    let path = Path::new(input);
    if path.is_relative() {
        base_dir.join(path)
    } else {
        path.to_path_buf()
    }
}

/// Get a path relative to cwd for display
pub fn get_cwd_relative_path(file_path: &Path, cwd: &Path) -> Option<String> {
    let relative = file_path.strip_prefix(cwd).ok()?;
    let rel_str = relative.to_string_lossy().to_string();
    if rel_str.is_empty() { Some(".".to_string()) } else { Some(rel_str) }
}

/// Format path relative to cwd, falling back to absolute
pub fn format_path_relative_to_cwd_or_absolute(file_path: &Path, cwd: &Path) -> String {
    get_cwd_relative_path(file_path, cwd)
        .unwrap_or_else(|| file_path.to_string_lossy().to_string())
}
