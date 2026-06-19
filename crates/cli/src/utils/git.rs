//! Git URL parsing utilities

/// Parsed git source information
#[derive(Debug, Clone)]
pub struct GitSource {
    /// Clone URL
    pub repo: String,
    /// Git host domain
    pub host: String,
    /// Repository path
    pub path: String,
    /// Git ref (branch/tag/commit)
    pub r#ref: Option<String>,
    /// Whether ref was specified
    pub pinned: bool,
}

/// Parse a git URL into its components
pub fn parse_git_url(source: &str) -> Option<GitSource> {
    let trimmed = source.trim();
    let has_git_prefix = trimmed.starts_with("git:");
    let url = if has_git_prefix {
        trimmed[4..].trim()
    } else {
        trimmed
    };

    // Must have a protocol if no git: prefix
    if !has_git_prefix && !url.contains("://") && !url.starts_with("git@") {
        return None;
    }

    let (repo, ref_str) = split_ref(url);

    let (host, path) = if let Some(scp) = repo.strip_prefix("git@") {
        let parts: Vec<&str> = scp.splitn(2, ':').collect();
        (
            parts[0].to_string(),
            parts.get(1).unwrap_or(&"").to_string(),
        )
    } else if repo.contains("://") {
        let parsed = url::Url::parse(&repo).ok()?;
        let host_str = parsed.host_str()?.to_string();
        let path_str = parsed.path().trim_start_matches('/').to_string();
        (host_str, path_str)
    } else {
        let parts: Vec<&str> = repo.splitn(2, '/').collect();
        (
            parts[0].to_string(),
            parts.get(1).unwrap_or(&"").to_string(),
        )
    };

    let normalized_path = path.trim_end_matches(".git").to_string();
    if host.is_empty() || normalized_path.is_empty() {
        return None;
    }

    Some(GitSource {
        repo: repo.trim_end_matches(".git").to_string(),
        host,
        path: normalized_path,
        r#ref: ref_str.clone(),
        pinned: ref_str.is_some(),
    })
}

/// Split a git URL into repo and ref components
fn split_ref(url: &str) -> (String, Option<String>) {
    // Handle SCP-like syntax: git@host:path@ref
    if let Some(scp) = url.strip_prefix("git@")
        && let Some(colon_idx) = scp.find(':')
    {
        let after_colon = &scp[colon_idx + 1..];
        if let Some(at_idx) = after_colon.rfind('@') {
            let repo = format!("git@{}:{}", &scp[..colon_idx], &after_colon[..at_idx]);
            return (repo, Some(after_colon[at_idx + 1..].to_string()));
        }
    }

    // Handle URL-like syntax
    if url.contains("://")
        && let Some(at_idx) = url.rfind('@')
        && let Some(slash_before_at) = url[..at_idx].rfind('/')
    {
        let repo = format!(
            "{}{}",
            &url[..=slash_before_at],
            &url[slash_before_at + 1..at_idx]
        );
        return (repo, Some(url[at_idx + 1..].to_string()));
    }

    // Handle host/path@ref
    if let Some(at_idx) = url.rfind('@')
        && let Some(_slash_idx) = url[..at_idx].find('/')
    {
        let repo = url[..at_idx].to_string();
        return (repo, Some(url[at_idx + 1..].to_string()));
    }

    (url.to_string(), None)
}
