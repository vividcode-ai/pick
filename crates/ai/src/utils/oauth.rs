//! OAuth utilities for AI providers

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ============================================================================
// Types
// ============================================================================

/// OAuth credentials for a provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthCredentials {
    /// Refresh token
    pub refresh: String,
    /// Access token (API key)
    pub access: String,
    /// Expiry timestamp in milliseconds
    pub expires: i64,
}

/// Information shown to the user during OAuth login
#[derive(Debug, Clone)]
pub struct OAuthAuthInfo {
    pub url: String,
    pub instructions: Option<String>,
}

/// Device code info for OAuth device code flow
#[derive(Debug, Clone)]
pub struct OAuthDeviceCodeInfo {
    pub user_code: String,
    pub verification_uri: String,
    pub interval_seconds: Option<u64>,
    pub expires_in_seconds: Option<u64>,
}

/// A prompt to show the user
#[derive(Debug, Clone)]
pub struct OAuthPrompt {
    pub message: String,
    pub placeholder: Option<String>,
    pub allow_empty: Option<bool>,
}

/// Result from polling an OAuth device code endpoint
#[derive(Debug, Clone)]
pub enum OAuthDeviceCodePollResult {
    Pending,
    SlowDown,
    Complete { access_token: String },
    Failed { message: String },
}

/// Options for polling an OAuth device code flow
pub struct OAuthDeviceCodePollOptions<F> {
    pub interval_seconds: Option<u64>,
    pub expires_in_seconds: Option<u64>,
    pub poll: F,
}

impl<F> OAuthDeviceCodePollOptions<F> {
    pub fn new(poll: F) -> Self {
        Self {
            interval_seconds: None,
            expires_in_seconds: None,
            poll,
        }
    }
}

/// Select option for OAuth selection prompts
#[derive(Debug, Clone)]
pub struct OAuthSelectOption {
    pub id: String,
    pub label: String,
}

// ============================================================================
// PKCE
// ============================================================================

const PKCE_VERIFIER_CHARS: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";

/// Generate a PKCE code verifier.
fn generate_pkce_verifier() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let input = format!(
        "{}-{}-{}",
        now.as_nanos(),
        now.as_micros(),
        std::process::id()
    );
    let hash = Sha256::digest(input.as_bytes());
    let mut verifier = String::with_capacity(64);
    for &byte in hash.iter().cycle().take(64) {
        let idx = (byte as usize) % PKCE_VERIFIER_CHARS.len();
        verifier.push(PKCE_VERIFIER_CHARS[idx] as char);
    }
    verifier
}

/// Generate PKCE code verifier and SHA-256 challenge.
pub fn generate_pkce() -> (String, String) {
    let verifier = generate_pkce_verifier();
    let hash = Sha256::digest(verifier.as_bytes());
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let challenge = engine.encode(hash);
    (verifier, challenge)
}

// ============================================================================
// Device Code Flow
// ============================================================================

const _CANCEL_MESSAGE: &str = "Login cancelled";
const TIMEOUT_MESSAGE: &str = "Device flow timed out";
const SLOW_DOWN_TIMEOUT_MESSAGE: &str = "Device flow timed out after one or more slow_down responses. \
     This is often caused by clock drift in WSL or VM environments. \
     Please sync or restart the VM clock and try again.";
const MINIMUM_INTERVAL_MS: u64 = 1000;
const DEFAULT_POLL_INTERVAL_SECONDS: u64 = 5;
const SLOW_DOWN_INTERVAL_INCREMENT_MS: u64 = 5000;

/// Poll an OAuth device code flow until completion or timeout.
///
/// `poll` is an async function that returns `OAuthDeviceCodePollResult`.
/// The function handles timing, slow_down responses, and timeout.
pub async fn poll_oauth_device_code_flow<F, Fut>(
    options: OAuthDeviceCodePollOptions<F>,
) -> Result<String, String>
where
    F: Fn() -> Fut + Send,
    Fut: std::future::Future<Output = OAuthDeviceCodePollResult> + Send,
{
    let deadline = match options.expires_in_seconds {
        Some(secs) => std::time::Instant::now() + std::time::Duration::from_secs(secs),
        None => std::time::Instant::now() + std::time::Duration::from_secs(300),
    };

    let mut interval_ms = std::cmp::max(
        MINIMUM_INTERVAL_MS,
        options
            .interval_seconds
            .unwrap_or(DEFAULT_POLL_INTERVAL_SECONDS)
            * 1000,
    );

    let mut slow_down_responses = 0;

    while std::time::Instant::now() < deadline {
        let remaining_ms = deadline
            .saturating_duration_since(std::time::Instant::now())
            .as_millis() as u64;
        let sleep_ms = std::cmp::min(interval_ms, remaining_ms);
        tokio::time::sleep(std::time::Duration::from_millis(sleep_ms)).await;

        let result = (options.poll)().await;

        match result {
            OAuthDeviceCodePollResult::Complete { access_token } => return Ok(access_token),
            OAuthDeviceCodePollResult::Pending => continue,
            OAuthDeviceCodePollResult::SlowDown => {
                slow_down_responses += 1;
                interval_ms = std::cmp::max(
                    MINIMUM_INTERVAL_MS,
                    interval_ms + SLOW_DOWN_INTERVAL_INCREMENT_MS,
                );
                continue;
            }
            OAuthDeviceCodePollResult::Failed { message } => return Err(message),
        }
    }

    if slow_down_responses > 0 {
        Err(SLOW_DOWN_TIMEOUT_MESSAGE.to_string())
    } else {
        Err(TIMEOUT_MESSAGE.to_string())
    }
}
