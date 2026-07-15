//! Project management — tracks historically used working directories.
//! Persisted to `~/.pick/projects.json`.

use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub path: String,
    pub name: String,
    #[serde(default)]
    pub last_used_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ProjectStore {
    #[serde(default)]
    projects: Vec<ProjectEntry>,
}

pub struct ProjectManager {
    current_cwd: RwLock<PathBuf>,
    store_path: PathBuf,
}

impl ProjectManager {
    pub fn new(initial_cwd: PathBuf) -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let store_path = home.join(".pick").join("projects.json");
        Self {
            current_cwd: RwLock::new(initial_cwd),
            store_path,
        }
    }

    /// Get the current (active) working directory.
    pub fn get_cwd(&self) -> PathBuf {
        self.current_cwd.read().unwrap().clone()
    }

    /// Switch the active working directory and record it as a project.
    pub fn set_cwd(&self, path: &Path) -> std::io::Result<PathBuf> {
        let canonical = path.canonicalize()?;
        {
            let mut cwd = self.current_cwd.write().unwrap();
            *cwd = canonical.clone();
        }
        self.add_project(&canonical)?;
        Ok(canonical)
    }

    /// Add a directory to the project history (updates last_used_at).
    pub fn add_project(&self, path: &Path) -> std::io::Result<()> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let name = canonical
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| canonical.to_string_lossy().to_string());
        let now = chrono::Utc::now().timestamp_millis();

        let mut store = self.load_store()?;

        // Update existing or add new
        if let Some(existing) = store
            .projects
            .iter_mut()
            .find(|p| p.path == canonical.to_string_lossy().as_ref())
        {
            existing.last_used_at = now;
        } else {
            store.projects.push(ProjectEntry {
                path: canonical.to_string_lossy().to_string(),
                name,
                last_used_at: now,
            });
        }

        self.save_store(&store)
    }

    /// Get all known project paths (for loading sessions at startup).
    pub fn get_all_project_paths(&self) -> Vec<PathBuf> {
        match self.load_store() {
            Ok(store) => store
                .projects
                .iter()
                .map(|p| PathBuf::from(&p.path))
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List all known projects, sorted by last_used_at descending.
    pub fn list_projects(&self) -> std::io::Result<Vec<ProjectEntry>> {
        let mut store = self.load_store()?;
        store
            .projects
            .sort_by_key(|b| std::cmp::Reverse(b.last_used_at));
        Ok(store.projects)
    }

    /// Remove a project from history by path.
    pub fn remove_project(&self, path: &str) -> std::io::Result<()> {
        let mut store = self.load_store()?;
        store.projects.retain(|p| p.path != path);
        self.save_store(&store)
    }

    fn load_store(&self) -> std::io::Result<ProjectStore> {
        if !self.store_path.exists() {
            return Ok(ProjectStore::default());
        }
        let content = std::fs::read_to_string(&self.store_path)?;
        serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    fn save_store(&self, store: &ProjectStore) -> std::io::Result<()> {
        if let Some(parent) = self.store_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(store)?;
        std::fs::write(&self.store_path, content)?;
        Ok(())
    }
}
