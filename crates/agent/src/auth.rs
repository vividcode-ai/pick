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

/// Path to the default auth.json file (~/.pick/agent/auth.json)
pub fn default_auth_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_default();
    home.join(".pick").join("agent").join("auth.json")
}

/// Read auth storage from a JSON file.
pub fn read_auth(path: &std::path::Path) -> anyhow::Result<AuthStorageData> {
    let content = std::fs::read_to_string(path)?;
    let data: AuthStorageData = serde_json::from_str(&content)?;
    Ok(data)
}

/// Write auth storage to a JSON file, creating parent directories if needed.
pub fn write_auth(path: &std::path::Path, data: &AuthStorageData) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(data)?;
    std::fs::write(path, content)?;
    Ok(())
}
