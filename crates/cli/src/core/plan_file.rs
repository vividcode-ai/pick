use std::path::{Path, PathBuf};

const PLANS_DIR: &str = ".pick/plans";

pub struct PlanFile {
    pub path: PathBuf,
    pub exists: bool,
}

impl PlanFile {
    pub fn new(cwd: &Path, session_id: &str) -> Self {
        let path = cwd.join(PLANS_DIR).join(format!("{}.md", session_id));
        let exists = path.exists();
        Self { path, exists }
    }

    pub fn plans_dir(cwd: &Path) -> PathBuf {
        cwd.join(PLANS_DIR)
    }

    pub fn create_dir(cwd: &Path) -> std::io::Result<()> {
        let dir = Self::plans_dir(cwd);
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }
        Ok(())
    }

    pub fn read(&self) -> std::io::Result<String> {
        std::fs::read_to_string(&self.path)
    }

    pub fn write(&self, content: &str) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent()
            && !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        std::fs::write(&self.path, content)
    }

    pub fn relative_to(&self, cwd: &Path) -> String {
        pathdiff::diff_paths(&self.path, cwd)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| self.path.to_string_lossy().replace('\\', "/"))
    }
}
