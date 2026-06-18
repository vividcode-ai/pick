use std::collections::HashMap;

/// OAuth credential with refresh token
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OAuthCredentials {
    pub refresh_token: String,
    pub access_token: String,
    pub expires_at: i64, // unix timestamp in seconds
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// OAuth provider ID
pub type OAuthProviderId = String;

/// Callbacks for OAuth login flow
pub struct OAuthLoginCallbacks {
    pub on_auth_url: Box<dyn Fn(&str) + Send + Sync>,
    pub on_device_code: Box<dyn Fn(&str, &str) + Send + Sync>,
    pub on_prompt: Box<dyn Fn(&str) -> Option<String> + Send + Sync>,
    pub on_select: Box<dyn Fn(&str, &[OAuthSelectOption]) -> Option<String> + Send + Sync>,
    pub signal: Option<tokio::sync::watch::Receiver<bool>>,
}

/// Option for select prompts
#[derive(Debug, Clone)]
pub struct OAuthSelectOption {
    pub id: String,
    pub label: String,
}

/// OAuth provider interface
#[async_trait::async_trait]
pub trait OAuthProvider: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    async fn login(&self, callbacks: &OAuthLoginCallbacks) -> Result<OAuthCredentials, String>;
    async fn refresh_token(&self, credentials: &OAuthCredentials) -> Result<OAuthCredentials, String>;
    fn get_api_key(&self, credentials: &OAuthCredentials) -> Option<String>;
}
