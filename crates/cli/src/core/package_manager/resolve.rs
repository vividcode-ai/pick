//! Package resolution and path parsing utilities

use std::collections::HashSet;
use std::path::Path;

use super::types::*;

// ============================================================================
// Ignore pattern matching
// ============================================================================

pub(crate) const IGNORE_FILE_NAMES: [&str; 3] = [".gitignore", ".ignore", ".fdignore"];

/// Simple ignore pattern matcher (replaces the `ignore` npm package)
pub(crate) struct IgnoreMatcher {
    patterns: Vec<(String, bool)>, // (pattern, is_negation)
}

impl IgnoreMatcher {
    pub(crate) fn new() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    pub(crate) fn add(&mut self, patterns: &[String]) {
        for p in patterns {
            let negated = p.starts_with('!');
            let pattern = if negated { &p[1..] } else { p.as_str() };
            self.patterns.push((pattern.to_string(), negated));
        }
    }

    pub(crate) fn ignores(&self, path: &str) -> bool {
        let posix_path = path.replace('\\', "/");
        let mut ignored = false;

        for (pattern, is_negation) in &self.patterns {
            if *is_negation {
                if self.matches(&posix_path, pattern) {
                    ignored = false;
                }
            } else if self.matches(&posix_path, pattern) {
                ignored = true;
            }
        }

        ignored
    }

    pub(crate) fn matches(&self, path: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        // Simple glob matching
        let pattern = pattern.trim_end_matches('/');
        let path = path.trim_end_matches('/');

        if path == pattern {
            return true;
        }

        // Check if pattern matches as a suffix (dir/** pattern)
        if pattern.ends_with("/*") {
            let base = &pattern[..pattern.len() - 2];
            return path == base || path.starts_with(&format!("{}/", base));
        }

        // Check if pattern matches as globstar (**/pattern)
        if pattern.starts_with("**/") {
            let suffix = &pattern[3..];
            return path == suffix || path.ends_with(&format!("/{}", suffix));
        }

        // Simple wildcard
        if pattern.contains('*') {
            let re_pattern = format!("^{}$", regex::escape(pattern).replace("\\*", ".*"));
            if let Ok(re) = regex::Regex::new(&re_pattern) {
                return re.is_match(path);
            }
        }

        path.contains(&format!("/{}", pattern))
    }
}

pub(crate) fn prefix_ignore_pattern(line: &str, prefix: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('#') && !trimmed.starts_with("\\#") {
        return None;
    }

    let mut pattern = trimmed.to_string();
    let negated = pattern.starts_with('!');

    if negated {
        let rest = &pattern[1..];
        let rest = if rest.starts_with('/') {
            &rest[1..]
        } else {
            rest
        };
        let prefixed = if prefix.is_empty() {
            rest.to_string()
        } else {
            format!("{}{}", prefix, rest)
        };
        return Some(format!("!{}", prefixed));
    }

    if pattern.starts_with('\\') {
        pattern = pattern[1..].to_string();
    }

    if pattern.starts_with('/') {
        pattern = pattern[1..].to_string();
    }

    let result = if prefix.is_empty() {
        pattern
    } else {
        format!("{}{}", prefix, pattern)
    };

    Some(result)
}

pub(crate) fn add_ignore_rules(ig: &mut IgnoreMatcher, dir: &Path, root_dir: &Path) {
    let relative_dir = pathdiff::diff_paths(dir, root_dir).unwrap_or_else(|| dir.to_path_buf());
    let prefix = if relative_dir.to_string_lossy().is_empty() {
        String::new()
    } else {
        format!("{}/", relative_dir.to_string_lossy().replace('\\', "/"))
    };

    for filename in &IGNORE_FILE_NAMES {
        let ignore_path = dir.join(filename);
        if !ignore_path.exists() {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&ignore_path) {
            let patterns: Vec<String> = content
                .split(|c: char| c == '\r' || c == '\n')
                .filter_map(|line| prefix_ignore_pattern(line, &prefix))
                .collect();
            if !patterns.is_empty() {
                ig.add(&patterns);
            }
        }
    }
}

// ============================================================================
// File collection
// ============================================================================

pub(crate) const SKIP_NODE_MODULES: &str = "node_modules";

pub(crate) fn collect_files(
    dir: &Path,
    file_pattern: &regex::Regex,
    skip_node_modules: bool,
    mut ignore_matcher: Option<&mut IgnoreMatcher>,
    root_dir: Option<&Path>,
) -> Vec<String> {
    let mut files = Vec::new();
    if !dir.exists() {
        return files;
    }

    let root = root_dir.unwrap_or(dir);

    // Create local ignore matcher at the top level when no parent matcher is provided
    let mut local_ig = IgnoreMatcher::new();
    if ignore_matcher.is_none() {
        add_ignore_rules(&mut local_ig, dir, root);
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return files,
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if skip_node_modules && name == SKIP_NODE_MODULES {
            continue;
        }

        let full_path = entry.path();
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let is_dir = metadata.is_dir() || metadata.file_type().is_symlink() && full_path.is_dir();
        let is_file =
            metadata.is_file() || metadata.file_type().is_symlink() && full_path.is_file();

        let rel_path = pathdiff::diff_paths(&full_path, root)
            .unwrap_or_else(|| full_path.clone())
            .to_string_lossy()
            .to_string()
            .replace('\\', "/");

        let ignore_path = if is_dir {
            format!("{}/", rel_path)
        } else {
            rel_path.clone()
        };

        // Check against parent's ignore matcher, or local one at top level
        let is_ignored = match ignore_matcher.as_mut() {
            Some(im) => im.ignores(&ignore_path),
            None => local_ig.ignores(&ignore_path),
        };
        if is_ignored {
            continue;
        }

        if is_dir {
            files.extend(collect_files(
                &full_path,
                file_pattern,
                skip_node_modules,
                ignore_matcher.as_mut().map(|r| &mut **r),
                Some(root),
            ));
        } else if is_file && file_pattern.is_match(&name) {
            files.push(full_path.to_string_lossy().to_string());
        }
    }

    files
}

pub(crate) fn collect_skill_entries(
    dir: &Path,
    mode: &str, // "skills" or "agents"
    mut ignore_matcher: Option<&mut IgnoreMatcher>,
    root_dir: Option<&Path>,
) -> Vec<String> {
    let mut entries = Vec::new();
    if !dir.exists() {
        return entries;
    }

    let root = root_dir.unwrap_or(dir);

    let mut local_ig = IgnoreMatcher::new();
    if ignore_matcher.is_none() {
        add_ignore_rules(&mut local_ig, dir, root);
    }

    let dir_entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(e) => e.flatten().collect(),
        Err(_) => return entries,
    };

    // Check if this directory contains SKILL.md
    for entry in &dir_entries {
        let name = entry.file_name().to_string_lossy().to_string();

        if name == "SKILL.md" {
            let full_path = entry.path();
            let is_file = entry.metadata().map(|m| m.is_file()).unwrap_or(false)
                || (entry
                    .metadata()
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false)
                    && full_path.is_file());

            let rel_path = pathdiff::diff_paths(&full_path, root)
                .unwrap_or_else(|| full_path.clone())
                .to_string_lossy()
                .to_string()
                .replace('\\', "/");

            if is_file {
                let is_ignored = match ignore_matcher.as_mut() {
                    Some(im) => im.ignores(&rel_path),
                    None => local_ig.ignores(&rel_path),
                };
                if !is_ignored {
                    entries.push(full_path.to_string_lossy().to_string());
                    return entries;
                }
            }
        }
    }

    // Recurse into subdirectories
    for entry in &dir_entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" {
            continue;
        }

        let full_path = entry.path();
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let is_dir = metadata.is_dir() || (metadata.file_type().is_symlink() && full_path.is_dir());
        if !is_dir {
            // For "skills" mode, top-level .md files are also skills
            if mode == "skills" && dir == root && metadata.is_file() && name.ends_with(".md") {
                let rel_path = pathdiff::diff_paths(&full_path, root)
                    .unwrap_or_else(|| full_path.clone())
                    .to_string_lossy()
                    .to_string()
                    .replace('\\', "/");

                let is_ignored = match ignore_matcher.as_mut() {
                    Some(im) => im.ignores(&rel_path),
                    None => local_ig.ignores(&rel_path),
                };
                if !is_ignored {
                    entries.push(full_path.to_string_lossy().to_string());
                }
            }
            continue;
        }

        let rel_path = pathdiff::diff_paths(&full_path, root)
            .unwrap_or_else(|| full_path.clone())
            .to_string_lossy()
            .to_string()
            .replace('\\', "/");

        let is_ignored = match ignore_matcher.as_mut() {
            Some(im) => im.ignores(&format!("{}/", rel_path)),
            None => local_ig.ignores(&format!("{}/", rel_path)),
        };
        if is_ignored {
            continue;
        }

        entries.extend(collect_skill_entries(
            &full_path,
            mode,
            ignore_matcher.as_mut().map(|r| &mut **r),
            Some(root),
        ));
    }

    entries
}

// ============================================================================
// Source parsing
// ============================================================================

pub(crate) fn is_local_path(source: &str) -> bool {
    source.starts_with('.') || source.starts_with('~') || Path::new(source).is_absolute()
}

pub(crate) fn parse_npm_spec(spec: &str) -> (String, Option<String>) {
    let re = regex::Regex::new(r"^(@?[^@]+(?:/[^@]+)?)(?:@(.+))?$").unwrap();
    if let Some(caps) = re.captures(spec) {
        let name = caps.get(1).map(|m| m.as_str()).unwrap_or(spec).to_string();
        let version = caps.get(2).map(|m| m.as_str().to_string());
        (name, version)
    } else {
        (spec.to_string(), None)
    }
}

pub(crate) fn parse_git_url(source: &str) -> Option<GitSource> {
    // GitHub shorthand: user/repo
    let re_github = regex::Regex::new(r"^([\w.-]+)/([\w.-]+)(?:@(.+))?$").unwrap();
    if let Some(caps) = re_github.captures(source) {
        let user = caps.get(1).unwrap().as_str();
        let repo = caps.get(2).unwrap().as_str();
        let r#ref = caps.get(3).map(|m| m.as_str().to_string());
        return Some(GitSource {
            repo: format!("https://github.com/{}/{}.git", user, repo),
            host: "github.com".to_string(),
            path_: format!("{}/{}", user, repo),
            r#ref,
        });
    }

    // Full git URL
    let re_full =
        regex::Regex::new(r"^(?:git@|https?://)([^:/]+)[:/](.+?)(?:\.git)?(?:@(.+))?$").unwrap();
    if let Some(caps) = re_full.captures(source) {
        let host = caps.get(1).unwrap().as_str().to_string();
        let path_ = caps.get(2).unwrap().as_str().to_string();
        let r#ref = caps.get(3).map(|m| m.as_str().to_string());
        let repo = if source.starts_with("git@") {
            format!("git@{}:{}.git", host, path_)
        } else {
            format!("https://{}/{}.git", host, path_)
        };
        return Some(GitSource {
            repo,
            host,
            path_,
            r#ref,
        });
    }

    None
}

pub(crate) fn parse_source(source: &str) -> ParsedSource {
    if source.starts_with("npm:") {
        let spec = source[4..].trim().to_string();
        let (name, version) = parse_npm_spec(&spec);
        return ParsedSource::Npm(NpmSource {
            spec,
            name,
            pinned: version.is_some(),
        });
    }

    if is_local_path(source) {
        return ParsedSource::Local(LocalSource {
            path_: source.to_string(),
        });
    }

    // Try git URL
    if let Some(git) = parse_git_url(source) {
        return ParsedSource::Git(git);
    }

    ParsedSource::Local(LocalSource {
        path_: source.to_string(),
    })
}

// ============================================================================
// Pattern matching for resource filtering
// ============================================================================

pub(crate) fn is_pattern(s: &str) -> bool {
    s.starts_with('!')
        || s.starts_with('+')
        || s.starts_with('-')
        || s.contains('*')
        || s.contains('?')
}

pub(crate) fn is_override_pattern(s: &str) -> bool {
    s.starts_with('!') || s.starts_with('+') || s.starts_with('-')
}

fn matches_any_pattern(file_path: &str, patterns: &[String], base_dir: &str) -> bool {
    let rel = pathdiff::diff_paths(file_path, base_dir)
        .unwrap_or_else(|| Path::new(file_path).to_path_buf())
        .to_string_lossy()
        .to_string()
        .replace('\\', "/");

    let path = Path::new(file_path);
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default()
        .to_string();
    let is_skill_file = name == "SKILL.md";
    let parent_dir = if is_skill_file { path.parent() } else { None };
    let parent_rel = parent_dir
        .and_then(|p| pathdiff::diff_paths(p, base_dir))
        .map(|p| p.to_string_lossy().to_string().replace('\\', "/"));
    let parent_name = parent_dir.map(|p| {
        p.file_name()
            .map(|s| s.to_string_lossy())
            .unwrap_or_default()
            .to_string()
    });

    patterns.iter().any(|pattern| {
        let normalized = pattern.replace('\\', "/");

        // Direct match on relative path, filename, or absolute path
        if rel == normalized || name == normalized || file_path.replace('\\', "/") == normalized {
            return true;
        }

        // Glob matching
        let re_str = format!("^{}$", regex::escape(&normalized).replace("\\*", ".*"));
        if let Ok(re) = regex::Regex::new(&re_str) {
            if re.is_match(&rel) || re.is_match(&name) {
                return true;
            }
        }

        // For skill files, also match parent directory
        if is_skill_file {
            if let (Some(p_rel), Some(p_name)) = (parent_rel.as_ref(), parent_name.as_ref()) {
                if normalized == *p_rel || normalized == *p_name {
                    return true;
                }
                if p_rel.ends_with(&format!("/{}", normalized)) {
                    return true;
                }
            }
        }

        false
    })
}

fn matches_any_exact_pattern(file_path: &str, patterns: &[String], base_dir: &str) -> bool {
    if patterns.is_empty() {
        return false;
    }

    let rel = pathdiff::diff_paths(file_path, base_dir)
        .unwrap_or_else(|| Path::new(file_path).to_path_buf())
        .to_string_lossy()
        .to_string()
        .replace('\\', "/");

    let path = Path::new(file_path);
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default()
        .to_string();
    let is_skill_file = name == "SKILL.md";
    let parent_dir = if is_skill_file { path.parent() } else { None };
    let parent_rel = parent_dir
        .and_then(|p| pathdiff::diff_paths(p, base_dir))
        .map(|p| p.to_string_lossy().to_string().replace('\\', "/"));
    let parent_dir_posix = parent_dir.map(|p| p.to_string_lossy().to_string().replace('\\', "/"));

    patterns.iter().any(|pattern| {
        let normalized = normalize_exact_pattern(pattern);
        if normalized == rel || normalized == file_path.replace('\\', "/") {
            return true;
        }
        if is_skill_file {
            if let (Some(p_rel), Some(p_dir)) = (parent_rel.as_ref(), parent_dir_posix.as_ref()) {
                if normalized == *p_rel || normalized == *p_dir {
                    return true;
                }
            }
        }
        false
    })
}

fn normalize_exact_pattern(pattern: &str) -> String {
    let normalized = if pattern.starts_with("./") || pattern.starts_with(".\\") {
        pattern[2..].to_string()
    } else {
        pattern.to_string()
    };
    normalized.replace('\\', "/")
}

fn get_override_patterns(entries: &[String]) -> Vec<String> {
    entries
        .iter()
        .filter(|p| p.starts_with('!') || p.starts_with('+') || p.starts_with('-'))
        .cloned()
        .collect()
}

pub(crate) fn split_patterns(entries: &[String]) -> (Vec<String>, Vec<String>) {
    let mut plain = Vec::new();
    let mut patterns = Vec::new();
    for entry in entries {
        if is_pattern(entry) {
            patterns.push(entry.clone());
        } else {
            plain.push(entry.clone());
        }
    }
    (plain, patterns)
}

pub(crate) fn apply_patterns(
    all_paths: &[String],
    patterns: &[String],
    base_dir: &str,
) -> HashSet<String> {
    let mut includes: Vec<String> = Vec::new();
    let mut excludes: Vec<String> = Vec::new();
    let mut force_includes: Vec<String> = Vec::new();
    let mut force_excludes: Vec<String> = Vec::new();

    for p in patterns {
        if p.starts_with('+') {
            force_includes.push(p[1..].to_string());
        } else if p.starts_with('-') {
            force_excludes.push(p[1..].to_string());
        } else if p.starts_with('!') {
            excludes.push(p[1..].to_string());
        } else {
            includes.push(p.clone());
        }
    }

    // Step 1: Apply includes (or all if no includes)
    let mut result: Vec<String> = if includes.is_empty() {
        all_paths.to_vec()
    } else {
        all_paths
            .iter()
            .filter(|f| matches_any_pattern(f, &includes, base_dir))
            .cloned()
            .collect()
    };

    // Step 2: Apply excludes
    if !excludes.is_empty() {
        result.retain(|f| !matches_any_pattern(f, &excludes, base_dir));
    }

    // Step 3: Force-include
    if !force_includes.is_empty() {
        for fp in all_paths {
            if !result.contains(fp) && matches_any_exact_pattern(fp, &force_includes, base_dir) {
                result.push(fp.clone());
            }
        }
    }

    // Step 4: Force-exclude
    if !force_excludes.is_empty() {
        result.retain(|f| !matches_any_exact_pattern(f, &force_excludes, base_dir));
    }

    result.into_iter().collect()
}
