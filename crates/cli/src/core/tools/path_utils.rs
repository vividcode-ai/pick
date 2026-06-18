use std::path::Path;

/// Check if a path exists (sync)
pub fn path_exists_sync(file_path: &str) -> bool {
    Path::new(file_path).exists()
}

/// Check if a path exists (async)
pub async fn path_exists(file_path: &str) -> bool {
    tokio::fs::try_exists(file_path).await.unwrap_or(false)
}

/// Normalize a path string
pub fn expand_path(file_path: &str) -> String {
    let p = file_path.replace('\u{202F}', " ");
    if let Some(stripped) = p.strip_prefix('@') {
        stripped.to_string()
    } else {
        p
    }
}

/// Resolve a path relative to the given cwd
pub fn resolve_to_cwd(file_path: &str, cwd: &str) -> String {
    let expanded = expand_path(file_path);
    let path = Path::new(&expanded);
    if path.is_absolute() {
        // Still normalize
        dunce::canonicalize(path).unwrap_or(path.to_path_buf()).to_string_lossy().to_string()
    } else {
        let joined = Path::new(cwd).join(&expanded);
        dunce::canonicalize(&joined).unwrap_or(joined).to_string_lossy().to_string()
    }
}

/// Resolve a read path with macOS filename variant fallbacks
pub fn resolve_read_path(file_path: &str, cwd: &str) -> String {
    let resolved = resolve_to_cwd(file_path, cwd);

    if path_exists_sync(&resolved) {
        return resolved;
    }

    // Try macOS AM/PM variant
    let am_pm = try_macos_screenshot_path(&resolved);
    if am_pm != resolved && path_exists_sync(&am_pm) {
        return am_pm;
    }

    // Try NFD variant
    let nfd = try_nfd_variant(&resolved);
    if nfd != resolved && path_exists_sync(&nfd) {
        return nfd;
    }

    // Try curly quote variant
    let curly = try_curly_quote_variant(&resolved);
    if curly != resolved && path_exists_sync(&curly) {
        return curly;
    }

    // Combined NFD + curly quote
    let nfd_curly = try_curly_quote_variant(&nfd);
    if nfd_curly != resolved && path_exists_sync(&nfd_curly) {
        return nfd_curly;
    }

    resolved
}

/// Resolve a read path with async existence checks
pub async fn resolve_read_path_async(file_path: &str, cwd: &str) -> String {
    let resolved = resolve_to_cwd(file_path, cwd);

    if path_exists(&resolved).await {
        return resolved;
    }

    let am_pm = try_macos_screenshot_path(&resolved);
    if am_pm != resolved && path_exists(&am_pm).await {
        return am_pm;
    }

    let nfd = try_nfd_variant(&resolved);
    if nfd != resolved && path_exists(&nfd).await {
        return nfd;
    }

    let curly = try_curly_quote_variant(&resolved);
    if curly != resolved && path_exists(&curly).await {
        return curly;
    }

    let nfd_curly = try_curly_quote_variant(&nfd);
    if nfd_curly != resolved && path_exists(&nfd_curly).await {
        return nfd_curly;
    }

    resolved
}

fn try_macos_screenshot_path(file_path: &str) -> String {
    let narrow_nb_space = '\u{202F}';
    // Replace " AM." or " PM." with narrow NBSP variant
    let re = regex::Regex::new(r" (AM|PM)\.").unwrap();
    re.replace_all(file_path, |caps: &regex::Captures| {
        format!("{}{}.", narrow_nb_space, &caps[1])
    })
    .to_string()
}

fn try_nfd_variant(file_path: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    file_path.nfd().collect()
}

fn try_curly_quote_variant(file_path: &str) -> String {
    file_path.replace('\'', "\u{2019}")
}
