use serde::Serialize;
use std::collections::HashMap;
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

#[derive(Debug, Clone, Serialize, Default)]
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
            trimmed.strip_prefix("* ").unwrap_or(trimmed).to_string()
        })
        .collect()
}

/// Batch all tracked-file diffs into a single `git diff` call, then handle
/// untracked files by reading them directly. This is ~10-50x faster than
/// spawning N separate git processes.
pub fn get_git_diffs(cwd: &Path, base: Option<&str>) -> GitDiffsResponse {
    let info = get_git_info(cwd);
    let branch = info.branch.clone();

    if info.changes.is_empty() {
        return GitDiffsResponse {
            branch,
            files: Vec::new(),
        };
    }

    // Collect tracked paths (M/A/D/R) for a single batched git diff.
    // Untracked files (??) are handled separately by reading their content.
    let tracked: Vec<&GitChange> = info
        .changes
        .iter()
        .filter(|c| c.status.trim() != "??")
        .collect();

    // ── Single batched git diff for all tracked files ──
    let mut combined_patch = String::new();
    if !tracked.is_empty() {
        let mut args = vec!["diff".to_string(), "--unified=3".to_string()];
        if let Some(b) = base {
            args.push(format!("{}...HEAD", b));
        } else {
            args.push("HEAD".to_string());
        }
        args.push("--".to_string());
        for change in &tracked {
            args.push(change.path.clone());
        }
        combined_patch = run_git_vec(cwd, &args);
    }

    // Split combined diff output → per-file map
    let file_patches = split_combined_diff(&combined_patch);

    // ── Build entries ──
    let mut files: Vec<GitDiffEntry> = Vec::with_capacity(info.changes.len());

    for change in &info.changes {
        let patch = if change.status.trim() == "??" {
            get_untracked_patch(cwd, &change.path)
        } else {
            file_patches.get(&change.path).cloned().unwrap_or_default()
        };

        let has_binary_marker = patch.contains("Binary files");
        let is_empty = patch.trim().is_empty();

        let (additions, deletions) = if has_binary_marker || is_empty {
            (0, 0)
        } else {
            count_diff_changes(&patch)
        };

        files.push(GitDiffEntry {
            path: change.path.clone(),
            status: change.status.clone(),
            additions,
            deletions,
            patch,
            binary: has_binary_marker || is_empty,
        });
    }

    GitDiffsResponse { branch, files }
}

/// Split `git diff` output (which may contain multiple files) into a
/// path→patch map.  Each file section begins with `diff --git a/…`.
fn split_combined_diff(combined: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut current_file: Option<String> = None;
    let mut current_start = 0usize;
    let lines: Vec<&str> = combined.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("diff --git a/") {
            // Save previous file
            if let Some(file) = current_file.take() {
                let patch = lines[current_start..i].join("\n");
                if !patch.is_empty() {
                    map.insert(file, patch);
                }
            }
            // Extract filename from "diff --git a/<path> b/<path>"
            if let Some(rest) = line.strip_prefix("diff --git a/")
                && let Some(path) = rest.split(" b/").next()
            {
                current_file = Some(path.to_string());
                current_start = i;
            }
        }
    }
    // Last file
    if let Some(file) = current_file {
        let patch = lines[current_start..].join("\n");
        if !patch.is_empty() {
            map.insert(file, patch);
        }
    }

    map
}

/// Construct a synthetic diff for a new/untracked file by reading its content.
fn get_untracked_patch(cwd: &Path, path: &str) -> String {
    let full_path = cwd.join(path);
    match std::fs::read_to_string(&full_path) {
        Ok(text) => {
            let line_count = text.lines().count();
            let added_lines: Vec<String> = text.lines().map(|l| format!("+{l}")).collect();
            format!(
                "diff --git a/{path} b/{path}\n\
                 new file mode 100644\n\
                 --- /dev/null\n\
                 +++ b/{path}\n\
                 @@ -0,0 +1,{line_count} @@\n\
                 {}",
                added_lines.join("\n")
            )
        }
        Err(_) => String::new(),
    }
}

/// Get a single file's full diff (for progressive/on-demand loading).
pub fn get_git_single_diff(cwd: &Path, file: &str) -> GitDiffEntry {
    let info = get_git_info(cwd);
    let change = info.changes.iter().find(|c| c.path == file);

    let (status, patch) = match change {
        Some(c) if c.status.trim() == "??" => (c.status.clone(), get_untracked_patch(cwd, &c.path)),
        Some(c) => {
            let args = vec![
                "diff".to_string(),
                "--unified=3".to_string(),
                "HEAD".to_string(),
                "--".to_string(),
                c.path.clone(),
            ];
            (c.status.clone(), run_git_vec(cwd, &args))
        }
        None => (String::new(), String::new()),
    };

    let binary = patch.contains("Binary files") || patch.is_empty();
    let (additions, deletions) = if binary {
        (0, 0)
    } else {
        count_diff_changes(&patch)
    };

    GitDiffEntry {
        path: file.to_string(),
        status,
        additions,
        deletions,
        patch,
        binary,
    }
}

fn count_diff_changes(patch: &str) -> (usize, usize) {
    let mut additions = 0usize;
    let mut deletions = 0usize;
    for line in patch.lines() {
        if line.starts_with('+') && !line.starts_with("+++") {
            additions += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
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
    let mut cmd = std::process::Command::new("git");
    cmd.args(args).current_dir(cwd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd.output()
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
