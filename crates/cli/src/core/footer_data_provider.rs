//! Provides git branch and extension statuses for footer display


use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

/// Git metadata paths found by walking up from cwd
#[derive(Debug, Clone)]
struct GitPaths {
    repo_dir: String,
    common_git_dir: String,
    head_path: String,
}

/// Find git metadata paths by walking up from cwd.
fn find_git_paths(cwd: &str) -> Option<GitPaths> {
    let mut dir = cwd.to_string();
    loop {
        let git_path = Path::new(&dir).join(".git");
        if git_path.exists() {
            if git_path.is_file() {
                if let Ok(content) = std::fs::read_to_string(&git_path) {
                    let trimmed = content.trim();
                    if let Some(gitdir_val) = trimmed.strip_prefix("gitdir: ") {
                        let git_dir = if Path::new(gitdir_val).is_absolute() {
                            gitdir_val.to_string()
                        } else {
                            let parent = Path::new(&dir);
                            parent.join(gitdir_val).to_string_lossy().to_string()
                        };
                        let head_path = Path::new(&git_dir).join("HEAD");
                        if !head_path.exists() {
                            return None;
                        }
                        let common_dir_path = Path::new(&git_dir).join("commondir");
                        let common_git_dir = if common_dir_path.exists() {
                            if let Ok(content) = std::fs::read_to_string(&common_dir_path) {
                                let rel = content.trim();
                                if Path::new(rel).is_absolute() {
                                    rel.to_string()
                                } else {
                                    Path::new(&git_dir)
                                        .join(rel)
                                        .to_string_lossy()
                                        .to_string()
                                }
                            } else {
                                git_dir.clone()
                            }
                        } else {
                            git_dir.clone()
                        };
                        return Some(GitPaths {
                            repo_dir: dir,
                            common_git_dir,
                            head_path: head_path.to_string_lossy().to_string(),
                        });
                    }
                }
            } else if git_path.is_dir() {
                let head_path = git_path.join("HEAD");
                if !head_path.exists() {
                    return None;
                }
                return Some(GitPaths {
                    repo_dir: dir,
                    common_git_dir: git_path.to_string_lossy().to_string(),
                    head_path: head_path.to_string_lossy().to_string(),
                });
            }
        }
        let parent = Path::new(&dir).parent()?;
        let parent_str = parent.to_string_lossy().to_string();
        if parent_str == dir {
            return None;
        }
        dir = parent_str;
    }
}

/// Resolve git branch synchronously
fn resolve_branch_with_git_sync(repo_dir: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["--no-optional-locks", "symbolic-ref", "--quiet", "--short", "HEAD"])
        .current_dir(repo_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if branch.is_empty() { None } else { Some(branch) }
    } else {
        None
    }
}

/// Resolve git branch asynchronously
async fn resolve_branch_with_git_async(repo_dir: &str) -> Option<String> {
    let output = tokio::process::Command::new("git")
        .args(["--no-optional-locks", "symbolic-ref", "--quiet", "--short", "HEAD"])
        .current_dir(repo_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .await
        .ok()?;
    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if branch.is_empty() { None } else { Some(branch) }
    } else {
        None
    }
}

/// Provides git branch and extension statuses for footer display
pub struct FooterDataProvider {
    cwd: Mutex<String>,
    extension_statuses: Mutex<HashMap<String, String>>,
    cached_branch: Mutex<Option<Option<String>>>,
    git_paths: Mutex<Option<GitPaths>>,
    branch_change_callbacks: Mutex<Vec<Box<dyn Fn() + Send>>>,
    available_provider_count: Mutex<usize>,
}

impl FooterDataProvider {
    pub fn new(cwd: String) -> Self {
        let git_paths = find_git_paths(&cwd);
        Self {
            cwd: Mutex::new(cwd),
            extension_statuses: Mutex::new(HashMap::new()),
            cached_branch: Mutex::new(Some(git_paths.as_ref().and_then(|gp| resolve_branch_from_head(&gp.head_path)))),
            git_paths: Mutex::new(git_paths),
            branch_change_callbacks: Mutex::new(Vec::new()),
            available_provider_count: Mutex::new(0),
        }
    }

    /// Current git branch, None if not in repo, "detached" if detached HEAD
    pub fn get_git_branch(&self) -> Option<String> {
        let mut cache = self.cached_branch.lock().unwrap();
        if cache.is_none() {
            let git_paths = self.git_paths.lock().unwrap();
            *cache = Some(git_paths.as_ref().and_then(|gp| resolve_branch_from_head(&gp.head_path)));
        }
        cache.clone().flatten()
    }

    /// Extension status texts
    pub fn get_extension_statuses(&self) -> HashMap<String, String> {
        self.extension_statuses.lock().unwrap().clone()
    }

    /// Subscribe to git branch changes. Returns a handle for unsubscription.
    pub fn on_branch_change<F: Fn() + Send + 'static>(&self, callback: F) -> usize {
        let mut callbacks = self.branch_change_callbacks.lock().unwrap();
        let id = callbacks.len();
        callbacks.push(Box::new(callback));
        id
    }

    /// Remove a branch change subscription
    pub fn remove_branch_change_callback(&self, id: usize) {
        let mut callbacks = self.branch_change_callbacks.lock().unwrap();
        if id < callbacks.len() {
            // Explicitly drop the callback: unused_must_use requires consuming Fn trait objects
            let _ = callbacks.remove(id);
        }
    }

    /// Set extension status
    pub fn set_extension_status(&self, key: &str, text: Option<&str>) {
        let mut statuses = self.extension_statuses.lock().unwrap();
        match text {
            Some(t) => { statuses.insert(key.to_string(), t.to_string()); }
            None => { statuses.remove(key); }
        }
    }

    /// Clear extension statuses
    pub fn clear_extension_statuses(&self) {
        self.extension_statuses.lock().unwrap().clear();
    }

    /// Number of unique providers with available models
    pub fn get_available_provider_count(&self) -> usize {
        *self.available_provider_count.lock().unwrap()
    }

    /// Set available provider count
    pub fn set_available_provider_count(&self, count: usize) {
        *self.available_provider_count.lock().unwrap() = count;
    }

    /// Update working directory
    pub fn set_cwd(&self, cwd: &str) {
        let mut cwd_field = self.cwd.lock().unwrap();
        if *cwd_field == cwd {
            return;
        }
        *cwd_field = cwd.to_string();
        *self.cached_branch.lock().unwrap() = None;
        *self.git_paths.lock().unwrap() = find_git_paths(cwd);
        self.notify_branch_change();
    }

    /// Refresh git branch asynchronously
    pub async fn refresh_git_branch_async(&self) {
        let git_paths = self.git_paths.lock().unwrap().clone();
        let branch = match &git_paths {
            Some(gp) => resolve_branch_from_head_async(&gp.head_path).await,
            None => None,
        };

        let mut cache = self.cached_branch.lock().unwrap();
        if cache.as_ref().map(|c| c.as_deref()).flatten() != branch.as_deref() {
            *cache = Some(branch);
            drop(cache);
            self.notify_branch_change();
        } else {
            *cache = Some(branch);
        }
    }

    fn notify_branch_change(&self) {
        let callbacks = self.branch_change_callbacks.lock().unwrap();
        for cb in callbacks.iter() {
            cb();
        }
    }
}

fn resolve_branch_from_head(head_path: &str) -> Option<String> {
    let content = std::fs::read_to_string(head_path).ok()?;
    let trimmed = content.trim();
    if let Some(branch) = trimmed.strip_prefix("ref: refs/heads/") {
        if branch == ".invalid" {
            resolve_branch_with_git_sync(Path::new(head_path).parent()?.parent()?.to_string_lossy().as_ref())
                .or(Some("detached".to_string()))
        } else {
            Some(branch.to_string())
        }
    } else {
        Some("detached".to_string())
    }
}

async fn resolve_branch_from_head_async(head_path: &str) -> Option<String> {
    let content = tokio::fs::read_to_string(head_path).await.ok()?;
    let trimmed = content.trim();
    if let Some(branch) = trimmed.strip_prefix("ref: refs/heads/") {
        if branch == ".invalid" {
            let repo_dir = Path::new(head_path).parent()?.parent()?.to_string_lossy().to_string();
            resolve_branch_with_git_async(&repo_dir)
                .await
                .or(Some("detached".to_string()))
        } else {
            Some(branch.to_string())
        }
    } else {
        Some("detached".to_string())
    }
}
