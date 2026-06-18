//! CHANGELOG.md parser.

use std::path::{Path, PathBuf};

/// A parsed entry from CHANGELOG.md
#[derive(Debug, Clone)]
pub struct ChangelogEntry {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub content: String,
}

/// Parse changelog entries from a CHANGELOG.md file.
/// Scans for `##` headers and collects content until the next `##` or EOF.
pub fn parse_changelog(path: &Path) -> Vec<ChangelogEntry> {
    if !path.exists() {
        return vec![];
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut entries: Vec<ChangelogEntry> = Vec::new();
    let mut current_lines: Vec<String> = Vec::new();
    let mut current_version: Option<(u32, u32, u32)> = None;
    let re = regex::Regex::new(r"##\s+\[?(\d+)\.(\d+)\.(\d+)\]?").unwrap();

    for line in content.lines() {
        if line.starts_with("## ") {
            // Save previous entry if exists
            if let Some((major, minor, patch)) = current_version {
                if !current_lines.is_empty() {
                    entries.push(ChangelogEntry {
                        major,
                        minor,
                        patch,
                        content: current_lines.join("\n").trim().to_string(),
                    });
                }
            }

            // Try to parse version from this header
            if let Some(caps) = re.captures(line) {
                let major: u32 = caps[1].parse().unwrap_or(0);
                let minor: u32 = caps[2].parse().unwrap_or(0);
                let patch: u32 = caps[3].parse().unwrap_or(0);
                current_version = Some((major, minor, patch));
                current_lines = vec![line.to_string()];
            } else {
                current_version = None;
                current_lines.clear();
            }
        } else if current_version.is_some() {
            current_lines.push(line.to_string());
        }
    }

    // Save last entry
    if let Some((major, minor, patch)) = current_version {
        if !current_lines.is_empty() {
            entries.push(ChangelogEntry {
                major,
                minor,
                patch,
                content: current_lines.join("\n").trim().to_string(),
            });
        }
    }

    entries
}

/// Compare versions. Returns -1 if v1 < v2, 0 if equal, 1 if v1 > v2.
pub fn compare_versions(v1: &ChangelogEntry, v2: &ChangelogEntry) -> i32 {
    if v1.major != v2.major {
        return if v1.major > v2.major { 1 } else { -1 };
    }
    if v1.minor != v2.minor {
        return if v1.minor > v2.minor { 1 } else { -1 };
    }
    if v1.patch != v2.patch {
        return if v1.patch > v2.patch { 1 } else { -1 };
    }
    0
}

/// Get entries newer than the given version string (e.g. "1.2.3").
pub fn get_new_entries(entries: &[ChangelogEntry], last_version: &str) -> Vec<ChangelogEntry> {
    let parts: Vec<u32> = last_version
        .split('.')
        .take(3)
        .map(|p| p.parse().unwrap_or(0))
        .collect();
    let last = ChangelogEntry {
        major: parts.first().copied().unwrap_or(0),
        minor: parts.get(1).copied().unwrap_or(0),
        patch: parts.get(2).copied().unwrap_or(0),
        content: String::new(),
    };

    entries
        .iter()
        .filter(|entry| compare_versions(entry, &last) > 0)
        .cloned()
        .collect()
}

/// Get the path to the changelog file.
pub fn get_changelog_path() -> PathBuf {
    crate::config::get_agent_dir().join("CHANGELOG.md")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_changelog() {
        let dir = std::env::temp_dir().join("Pick-changelog-test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("CHANGELOG.md");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "## [1.0.0] - 2024-01-01").unwrap();
        writeln!(file, "- Initial release").unwrap();
        writeln!(file, "- Feature one").unwrap();
        writeln!(file, "## [0.9.0] - 2023-12-01").unwrap();
        writeln!(file, "- Beta release").unwrap();
        writeln!(file, "## 0.8.0").unwrap();
        writeln!(file, "- Alpha release").unwrap();
        drop(file);

        let entries = parse_changelog(&path);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].major, 1);
        assert_eq!(entries[0].minor, 0);
        assert_eq!(entries[0].patch, 0);
        assert!(entries[0].content.contains("Initial release"));
        assert_eq!(entries[1].major, 0);
        assert_eq!(entries[1].minor, 9);
        assert_eq!(entries[1].patch, 0);
        assert_eq!(entries[2].major, 0);
        assert_eq!(entries[2].minor, 8);
        assert_eq!(entries[2].patch, 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_nonexistent() {
        let entries = parse_changelog(Path::new("/nonexistent/CHANGELOG.md"));
        assert!(entries.is_empty());
    }

    #[test]
    fn test_compare_versions() {
        let v1 = ChangelogEntry {
            major: 1,
            minor: 0,
            patch: 0,
            content: String::new(),
        };
        let v2 = ChangelogEntry {
            major: 1,
            minor: 0,
            patch: 1,
            content: String::new(),
        };
        let v3 = ChangelogEntry {
            major: 2,
            minor: 0,
            patch: 0,
            content: String::new(),
        };

        assert_eq!(compare_versions(&v1, &v1), 0);
        assert_eq!(compare_versions(&v1, &v2), -1);
        assert_eq!(compare_versions(&v2, &v1), 1);
        assert_eq!(compare_versions(&v3, &v1), 1);
        assert_eq!(compare_versions(&v1, &v3), -1);
    }

    #[test]
    fn test_get_new_entries() {
        let entries = vec![
            ChangelogEntry {
                major: 1,
                minor: 0,
                patch: 0,
                content: "First".to_string(),
            },
            ChangelogEntry {
                major: 2,
                minor: 0,
                patch: 0,
                content: "Second".to_string(),
            },
            ChangelogEntry {
                major: 3,
                minor: 0,
                patch: 0,
                content: "Third".to_string(),
            },
        ];

        let new_entries = get_new_entries(&entries, "1.5.0");
        assert_eq!(new_entries.len(), 2);
        assert_eq!(new_entries[0].major, 2);
        assert_eq!(new_entries[1].major, 3);
    }
}
