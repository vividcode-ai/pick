use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct GitChange {
    pub path: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GitInfo {
    pub branch: String,
    pub changes: Vec<GitChange>,
    pub cwd: String,
}

pub fn get_git_info(cwd: &Path) -> GitInfo {
    let branch = run_git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let status_output = run_git(cwd, &["status", "--porcelain"]);

    let changes: Vec<GitChange> = if status_output.is_empty() {
        Vec::new()
    } else {
        status_output
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                let status = line[..2].trim().to_string();
                let path = if line.len() > 3 {
                    line[3..].to_string()
                } else {
                    String::new()
                };
                GitChange { path, status }
            })
            .collect()
    };

    GitInfo {
        branch,
        changes,
        cwd: cwd.to_string_lossy().to_string(),
    }
}

fn run_git(cwd: &Path, args: &[&str]) -> String {
    std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .unwrap_or_default()
        .trim()
        .to_string()
}
