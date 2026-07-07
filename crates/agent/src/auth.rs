//! Shared auth types and file I/O for AI provider credentials.
//! Used by both CLI and Web (server) to read/write `auth.json`.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AuthCredential {
    #[serde(rename = "api_key")]
    ApiKey { key: String },
    #[serde(rename = "oauth")]
    Oauth {
        #[serde(flatten)]
        inner: OAuthCredentials,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires: i64,
    pub token_type: String,
    pub scope: Option<Vec<String>>,
}

pub type AuthStorageData = HashMap<String, AuthCredential>;

/// Full content of auth.json, including credentials and last used model info.
/// The `credentials` field is flattened so credential entries (which have a `type` field)
/// appear as top-level keys alongside `last_provider`/`last_model`.
/// This is backward-compatible: old files without `last_*` fields simply leave them as `None`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthFile {
    #[serde(flatten)]
    pub credentials: HashMap<String, AuthCredential>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<String>,
}

/// Path to the default auth.json file (~/.pick/agent/auth.json)
pub fn default_auth_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_default();
    home.join(".pick").join("agent").join("auth.json")
}

/// Read the full auth file, including credentials and last used model info.
pub fn read_auth_file(path: &std::path::Path) -> anyhow::Result<AuthFile> {
    let content = std::fs::read_to_string(path)?;
    let data: AuthFile = serde_json::from_str(&content)?;
    Ok(data)
}

/// Write the full auth file, including credentials and last used model info.
pub fn write_auth_file(path: &std::path::Path, data: &AuthFile) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(data)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Read auth storage from a JSON file (credentials only, for backward compatibility).
pub fn read_auth(path: &std::path::Path) -> anyhow::Result<AuthStorageData> {
    let content = std::fs::read_to_string(path)?;
    // Try new AuthFile format first, fall back to old flat HashMap
    if let Ok(file) = serde_json::from_str::<AuthFile>(&content) {
        return Ok(file.credentials);
    }
    let data: AuthStorageData = serde_json::from_str(&content)?;
    Ok(data)
}

/// Write auth storage to a JSON file, creating parent directories if needed.
/// Note: This only writes credentials; use `write_auth_file` to also persist last model info.
pub fn write_auth(path: &std::path::Path, data: &AuthStorageData) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(data)?;
    std::fs::write(path, content)?;
    Ok(())
}
