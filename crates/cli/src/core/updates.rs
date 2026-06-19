//! Version checking for Pick auto-updates.

use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::{VERSION, get_agent_dir};
use crate::utils::user_agent::get_user_agent;
use crate::utils::version_check::is_newer_package_version;

use super::install_context::{InstallContext, InstallMethod};

const CACHE_REFRESH_HOURS: i64 = 20;
const GITHUB_API_URL: &str = "https://api.github.com/repos/vividcodeai/pick/releases/latest";
const NPM_REGISTRY_URL: &str = "https://registry.npmjs.org/@vividcodeai/pick";
const HTTP_TIMEOUT_MS: u64 = 10000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionCache {
    pub latest_version: String,
    pub last_checked_at: Option<String>,
    pub dismissed_version: Option<String>,
}

impl VersionCache {
    fn cache_path() -> PathBuf {
        get_agent_dir().join("version.json")
    }

    pub fn load() -> Option<Self> {
        let path = Self::cache_path();
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn save(&self) {
        let path = Self::cache_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, content);
        }
    }

    pub fn is_stale(&self) -> bool {
        let Some(ref checked) = self.last_checked_at else {
            return true;
        };
        let Ok(checked_dt) = checked.parse::<DateTime<Utc>>() else {
            return true;
        };
        let elapsed = Utc::now() - checked_dt;
        elapsed.num_hours() >= CACHE_REFRESH_HOURS
    }

    pub fn update(&mut self, version: &str) {
        self.latest_version = version.to_string();
        self.last_checked_at = Some(Utc::now().to_rfc3339());
    }

    pub fn dismiss_current(&mut self) {
        self.dismissed_version = Some(self.latest_version.clone());
    }
}

fn is_source_build(version: &str) -> bool {
    version == "0.0.0"
}

/// Fetch the latest version from GitHub releases.
async fn fetch_latest_from_github() -> Option<String> {
    let client = reqwest::Client::new();
    let response = client
        .get(GITHUB_API_URL)
        .header("User-Agent", get_user_agent(VERSION))
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_millis(HTTP_TIMEOUT_MS))
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let data: serde_json::Value = response.json().await.ok()?;
    let tag = data.get("tag_name")?.as_str()?;
    let version = tag.trim_start_matches('v').trim_start_matches("v");
    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

/// Fetch the latest version from npm registry.
async fn fetch_latest_from_npm() -> Option<String> {
    let client = reqwest::Client::new();
    let response = client
        .get(NPM_REGISTRY_URL)
        .header("User-Agent", get_user_agent(VERSION))
        .header("Accept", "application/vnd.npm.install-v1+json")
        .timeout(Duration::from_millis(HTTP_TIMEOUT_MS))
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let data: serde_json::Value = response.json().await.ok()?;
    let version = data
        .get("dist-tags")
        .and_then(|t| t.get("latest"))
        .and_then(|v| v.as_str())?;
    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

/// Fetch the latest version using the appropriate source for the install method.
pub async fn fetch_latest_version(ctx: &InstallContext) -> Option<String> {
    match ctx.method {
        InstallMethod::Npm => {
            // For npm, check both GitHub and npm registry for consistency
            let github_ver = fetch_latest_from_github().await;
            let npm_ver = fetch_latest_from_npm().await;
            // Prefer npm's version if it matches GitHub, otherwise whichever is newer
            match (github_ver, npm_ver) {
                (Some(g), Some(n)) if g == n => Some(n),
                (Some(g), _) => Some(g),
                (_, Some(n)) => Some(n),
                _ => None,
            }
        }
        _ => fetch_latest_from_github().await,
    }
}

/// Get the upgrade version, using cache if fresh enough.
/// Returns None if up-to-date, on error, or if suppressed.
pub async fn get_upgrade_version(check_dismissed: bool) -> Option<String> {
    // Skip for source builds
    if is_source_build(VERSION) {
        return None;
    }

    let mut cache = VersionCache::load().unwrap_or(VersionCache {
        latest_version: VERSION.to_string(),
        last_checked_at: None,
        dismissed_version: None,
    });

    // Check if dismissed
    if check_dismissed
        && let Some(ref dismissed) = cache.dismissed_version
            && dismissed == &cache.latest_version {
                return None;
            }

    // Refresh cache if stale
    if cache.is_stale() {
        let ctx = InstallContext::current();
        if let Some(latest) = fetch_latest_version(ctx).await {
            cache.update(&latest);
            cache.save();
        }
    }

    // Compare
    if is_newer_package_version(&cache.latest_version, VERSION) {
        Some(cache.latest_version.clone())
    } else {
        None
    }
}

/// Dismiss the current upgrade notification.
pub fn dismiss_version() {
    let mut cache = VersionCache::load().unwrap_or(VersionCache {
        latest_version: VERSION.to_string(),
        last_checked_at: None,
        dismissed_version: None,
    });
    cache.dismiss_current();
    cache.save();
}
