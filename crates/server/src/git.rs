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

#[derive(Debug, Clone, Serialize)]
pub struct GitDiffEntry {
    pub path: String,
    pub status: String,
    pub additions: usize,
    pub deletions: usize,
    pub patch: String,
    pub binary: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GitDiffsResponse {
    pub branch: String,
    pub files: Vec<GitDiffEntry>,
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

pub fn list_git_branches(cwd: &Path) -> Vec<String> {
    let output = run_git(cwd, &["branch", "-a"]);
    if output.is_empty() {
        return Vec::new();
    }
    output
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let trimmed = l.trim();
            // Remove leading * (current branch marker)
            trimmed.strip_prefix("* ").unwrap_or(trimmed).to_string()
        })
        .collect()
}

pub fn get_git_diffs(cwd: &Path, base: Option<&str>) -> GitDiffsResponse {
    let info = get_git_info(cwd);
    let branch = info.branch.clone();

    let files: Vec<GitDiffEntry> = info
        .changes
        .iter()
        .map(|change| get_file_diff(cwd, &change.path, &change.status, base))
        .collect();

    GitDiffsResponse { branch, files }
}

fn get_file_diff(cwd: &Path, path: &str, status: &str, base: Option<&str>) -> GitDiffEntry {
    let status_short = status.trim();

    let patch = match status_short {
        "M" | "D" | "R" => {
            let args = build_diff_args(path, base, false);
            run_git_vec(cwd, &args)
        }
        "A" => {
            let args = build_diff_args(path, base, true);
            run_git_vec(cwd, &args)
        }
        "?" | "??" => {
            // Untracked: show entire file as addition
            let content = run_git(cwd, &["show", "--", path]);
            if content.is_empty() {
                // Try reading file directly
                let full_path = cwd.join(path);
                match std::fs::read_to_string(&full_path) {
                    Ok(text) => format!(
                        "diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1,{} @@\n{}",
                        text.lines().count(),
                        text.lines()
                            .map(|l| format!("+{l}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                    Err(_) => String::new(),
                }
            } else {
                content
            }
        }
        _ => String::new(),
    };

    let binary = patch.contains("Binary files") || patch.is_empty();

    let (additions, deletions) = if binary {
        (0, 0)
    } else {
        count_diff_changes(&patch)
    };

    GitDiffEntry {
        path: path.to_string(),
        status: status.to_string(),
        additions,
        deletions,
        patch,
        binary,
    }
}

fn build_diff_args(path: &str, base: Option<&str>, cached: bool) -> Vec<String> {
    let mut args = vec!["diff".to_string(), "--unified=3".to_string()];
    if let Some(b) = base {
        args.push(format!("{b}...HEAD"));
    } else if cached {
        args.push("--cached".to_string());
    } else {
        args.push("HEAD".to_string());
    }
    args.push("--".to_string());
    args.push(path.to_string());
    args
}

fn count_diff_changes(patch: &str) -> (usize, usize) {
    let mut additions = 0usize;
    let mut deletions = 0usize;
    for line in patch.lines() {
        if line.starts_with("+") && !line.starts_with("+++") {
            additions += 1;
        } else if line.starts_with("-") && !line.starts_with("---") {
            deletions += 1;
        }
    }
    (additions, deletions)
}

fn run_git_vec(cwd: &Path, args: &[String]) -> String {
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_git(cwd, &refs)
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
