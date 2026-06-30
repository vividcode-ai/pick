use std::path::Path;

include!(concat!(env!("OUT_DIR"), "/embedded_docs_generated.rs"));

pub fn extract_embedded_docs(agent_dir: &Path) -> std::io::Result<()> {
    let version_file = agent_dir.join(".embedded_version");

    if version_file.exists() {
        if let Ok(current) = std::fs::read_to_string(&version_file) {
            if current.trim() == EMBEDDED_VERSION {
                return Ok(());
            }
        }
    }

    let docs_dir = agent_dir.join("docs");
    let examples_dir = agent_dir.join("examples");

    for file in EMBEDDED_FILES {
        let target_path = agent_dir.join(file.relative_path);
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target_path, file.content)?;
    }

    // Clean up stale files if versions don't match
    cleanup_stale(&docs_dir, "docs")?;
    cleanup_stale(&examples_dir, "examples")?;

    std::fs::write(&version_file, EMBEDDED_VERSION)?;

    tracing::info!(
        "Extracted embedded documentation ({} files, v{})",
        EMBEDDED_FILES.len(),
        EMBEDDED_VERSION
    );

    Ok(())
}

fn cleanup_stale(dir: &Path, prefix: &str) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    let embedded_paths: std::collections::HashSet<String> = EMBEDDED_FILES
        .iter()
        .filter(|f| f.relative_path.starts_with(prefix))
        .map(|f| f.relative_path.to_string())
        .collect();

    remove_stale_entries(dir, dir, &embedded_paths, prefix)?;
    Ok(())
}

fn remove_stale_entries(
    base: &Path,
    dir: &Path,
    keep: &std::collections::HashSet<String>,
    prefix: &str,
) -> std::io::Result<()> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            remove_stale_entries(base, &path, keep, prefix)?;
            if path.read_dir().map_or(false, |mut i| i.next().is_none()) {
                let _ = std::fs::remove_dir(&path);
            }
        } else if path.is_file() {
            if let Ok(relative) = path.strip_prefix(base) {
                let rel_str = format!(
                    "{}/{}",
                    prefix,
                    relative.to_string_lossy().replace('\\', "/")
                );
                if !keep.contains(&rel_str) {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }
    Ok(())
}
