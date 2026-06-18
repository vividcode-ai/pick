//! Version check utilities

use super::user_agent::get_user_agent;

const LATEST_VERSION_URL: &str = "https://pick.dev/api/latest-version";
const DEFAULT_TIMEOUT_MS: u64 = 10000;

/// Information about the latest release.
#[derive(Debug, Clone)]
pub struct LatestRelease {
    pub version: String,
    pub package_name: Option<String>,
    pub note: Option<String>,
}

fn parse_package_version(version: &str) -> Option<(u32, u32, u32, Option<String>)> {
    let version = version.trim();
    let re = regex::Regex::new(r"^v?(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+.*)?$").unwrap();
    let caps = re.captures(version)?;
    Some((
        caps[1].parse().unwrap_or(0),
        caps[2].parse().unwrap_or(0),
        caps[3].parse().unwrap_or(0),
        caps.get(4).map(|m| m.as_str().to_string()),
    ))
}

/// Compare two semver strings. Returns `None` if either is unparseable.
pub fn compare_package_versions(left: &str, right: &str) -> Option<i32> {
    let (lmaj, lmin, lpat, lpre) = parse_package_version(left)?;
    let (rmaj, rmin, rpat, rpre) = parse_package_version(right)?;

    if lmaj != rmaj {
        return Some(if lmaj > rmaj { 1 } else { -1 });
    }
    if lmin != rmin {
        return Some(if lmin > rmin { 1 } else { -1 });
    }
    if lpat != rpat {
        return Some(if lpat > rpat { 1 } else { -1 });
    }
    match (lpre, rpre) {
        (None, None) => Some(0),
        (Some(_), None) => Some(-1),
        (None, Some(_)) => Some(1),
        (Some(a), Some(b)) => Some(a.cmp(&b) as i32),
    }
}

/// Check if `candidate_version` is newer than `current_version`.
pub fn is_newer_package_version(candidate: &str, current: &str) -> bool {
    match compare_package_versions(candidate, current) {
        Some(cmp) => cmp > 0,
        None => candidate.trim() != current.trim(),
    }
}

/// Fetch the latest release info from the API.
pub async fn get_latest_release(
    current_version: &str,
    timeout_ms: Option<u64>,
) -> Option<LatestRelease> {
    if std::env::var("PICK_SKIP_VERSION_CHECK").is_ok()
        || std::env::var("PICK_OFFLINE").is_ok()
    {
        return None;
    }

    let client = reqwest::Client::new();
    let response = client
        .get(LATEST_VERSION_URL)
        .header("User-Agent", get_user_agent(current_version))
        .header("Accept", "application/json")
        .timeout(std::time::Duration::from_millis(
            timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS),
        ))
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let data: serde_json::Value = response.json().await.ok()?;
    let version = data.get("version")?.as_str()?;
    let version = version.trim();
    if version.is_empty() {
        return None;
    }

    let package_name = data
        .get("packageName")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string());
    let note = data
        .get("note")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string());

    Some(LatestRelease {
        version: version.to_string(),
        package_name,
        note,
    })
}

/// Get just the latest version string.
pub async fn get_latest_version(
    current_version: &str,
    timeout_ms: Option<u64>,
) -> Option<String> {
    get_latest_release(current_version, timeout_ms)
        .await
        .map(|r| r.version)
}

/// Check if a newer version is available. Returns `None` on failure or if up-to-date.
pub async fn check_for_new_version(
    current_version: &str,
) -> Option<LatestRelease> {
    let latest = get_latest_release(current_version, None).await?;
    if is_newer_package_version(&latest.version, current_version) {
        Some(latest)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_package_version() {
        let (maj, min, pat, pre) = parse_package_version("1.2.3").unwrap();
        assert_eq!((maj, min, pat), (1, 2, 3));
        assert!(pre.is_none());

        let (maj, min, pat, pre) = parse_package_version("v2.0.0-beta.1").unwrap();
        assert_eq!((maj, min, pat), (2, 0, 0));
        assert_eq!(pre.unwrap(), "beta.1");

        assert!(parse_package_version("invalid").is_none());
    }

    #[test]
    fn test_compare_package_versions() {
        assert_eq!(compare_package_versions("1.0.0", "1.0.0"), Some(0));
        assert_eq!(compare_package_versions("2.0.0", "1.0.0"), Some(1));
        assert_eq!(compare_package_versions("1.0.0", "2.0.0"), Some(-1));
        assert_eq!(compare_package_versions("1.0.0", "1.1.0"), Some(-1));
        assert_eq!(compare_package_versions("1.1.0", "1.0.0"), Some(1));
        assert_eq!(compare_package_versions("1.0.0", "Invalid"), None);
    }

    #[test]
    fn test_is_newer() {
        assert!(is_newer_package_version("2.0.0", "1.0.0"));
        assert!(!is_newer_package_version("1.0.0", "1.0.0"));
        assert!(!is_newer_package_version("0.9.0", "1.0.0"));
    }
}
