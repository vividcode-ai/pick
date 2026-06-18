//! OAuth authentication flows for AI providers

pub mod types;
pub use types::*;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

type OAuthProviderBox = Arc<dyn OAuthProvider>;

lazy_static::lazy_static! {
    static ref OAUTH_REGISTRY: std::sync::RwLock<HashMap<String, OAuthProviderBox>> = {
        let mut map = HashMap::new();
        map.insert("anthropic".to_string(), Arc::new(AnthropicOAuth) as OAuthProviderBox);
        map.insert("github-copilot".to_string(), Arc::new(GitHubCopilotOAuth) as OAuthProviderBox);
        map.insert("openai-codex".to_string(), Arc::new(OpenAICodexOAuth) as OAuthProviderBox);
        std::sync::RwLock::new(map)
    };
}

pub fn register_oauth_provider(id: &str, provider: OAuthProviderBox) {
    if let Ok(mut reg) = OAUTH_REGISTRY.write() {
        reg.insert(id.to_string(), provider);
    }
}

pub fn get_oauth_provider(id: &str) -> Option<OAuthProviderBox> {
    if let Ok(reg) = OAUTH_REGISTRY.read() {
        reg.get(id).cloned()
    } else {
        None
    }
}

pub fn unregister_oauth_provider(id: &str) {
    if let Ok(mut reg) = OAUTH_REGISTRY.write() {
        reg.remove(id);
    }
}

pub fn list_oauth_providers() -> Vec<String> {
    if let Ok(reg) = OAUTH_REGISTRY.read() {
        reg.keys().cloned().collect()
    } else {
        vec![]
    }
}

pub async fn refresh_oauth_token(
    provider_id: &str,
    credentials: &OAuthCredentials,
) -> Result<OAuthCredentials, String> {
    let provider = get_oauth_provider(provider_id)
        .ok_or_else(|| format!("No OAuth provider: {}", provider_id))?;
    provider.refresh_token(credentials).await
}

pub async fn get_oauth_api_key(
    provider_id: &str,
    credentials: &mut OAuthCredentials,
) -> Result<String, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    if credentials.expires_at < now + 300 {
        let new_creds = refresh_oauth_token(provider_id, credentials).await?;
        *credentials = new_creds;
    }

    let provider = get_oauth_provider(provider_id)
        .ok_or_else(|| format!("No OAuth provider: {}", provider_id))?;
    provider
        .get_api_key(credentials)
        .ok_or_else(|| "No API key in OAuth credentials".to_string())
}

pub fn generate_pkce() -> (String, String) {
    use sha2::{Digest, Sha256};

    let verifier_bytes: Vec<u8> = (0..32).map(|_| rand_byte()).collect();
    let verifier = base64_url_encode(&verifier_bytes);

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = base64_url_encode(&hasher.finalize());

    (verifier, challenge)
}

fn rand_byte() -> u8 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos ^ (nanos.wrapping_mul(1103515245).wrapping_add(12345) >> 16)) as u8
}

fn base64_url_encode(data: &[u8]) -> String {
    use base64::Engine;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    engine.encode(data)
}

pub async fn poll_device_code_flow(
    device_code: &str,
    interval_seconds: u64,
    expires_in_seconds: u64,
    token_url: &str,
    client_id: &str,
    on_progress: Option<&(dyn Fn() + Sync)>,
    signal: Option<tokio::sync::watch::Receiver<bool>>,
) -> Result<OAuthCredentials, String> {
    let start = Instant::now();
    let deadline = Duration::from_secs(expires_in_seconds);
    let mut interval = interval_seconds;

    let client = reqwest::Client::new();

    loop {
        if start.elapsed() >= deadline {
            return Err("Device code expired".to_string());
        }

        if let Some(ref sig) = signal {
            if *sig.borrow() {
                return Err("Login cancelled".to_string());
            }
        }

        tokio::time::sleep(Duration::from_secs(interval)).await;

        let params = [
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("device_code", device_code),
            ("client_id", client_id),
        ];

        match client.post(token_url).form(&params).send().await {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();

                if status.is_success() {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        let access_token = json
                            .get("access_token")
                            .and_then(|v| v.as_str())
                            .ok_or("Missing access_token")?
                            .to_string();
                        let refresh_token = json
                            .get("refresh_token")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let expires_in = json
                            .get("expires_in")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(3600);
                        let expires_at = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64
                            + expires_in as i64;

                        return Ok(OAuthCredentials {
                            refresh_token,
                            access_token,
                            expires_at,
                            extra: HashMap::new(),
                        });
                    }
                } else if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    let error = json.get("error").and_then(|v| v.as_str()).unwrap_or("");
                    match error {
                        "authorization_pending" => {
                            if let Some(ref cb) = on_progress {
                                cb();
                            }
                        }
                        "slow_down" => {
                            interval = (interval + 5).min(30);
                        }
                        "expired_token" => {
                            return Err("Device code expired".to_string());
                        }
                        "access_denied" => {
                            return Err("User denied authorization".to_string());
                        }
                        _ => {
                            return Err(format!("Device code error: {}: {}", error, text));
                        }
                    }
                } else {
                    return Err(format!("Device code HTTP {}: {}", status, text));
                }
            }
            Err(e) => {
                return Err(format!("Device code request failed: {}", e));
            }
        }
    }
}

pub async fn start_callback_server(
    port: u16,
    _path: &str,
    timeout_secs: u64,
) -> Result<String, String> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind callback server: {}", e))?;

    listener.set_ttl(1).ok();
    let (mut stream, _) =
        tokio::time::timeout(Duration::from_secs(timeout_secs), listener.accept())
            .await
            .map_err(|_| "Callback server timed out waiting for browser")?
            .map_err(|e| format!("Callback accept error: {}", e))?;

    use tokio::io::AsyncReadExt;
    let mut buf = vec![0u8; 4096];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| format!("Callback read error: {}", e))?;

    let request = String::from_utf8_lossy(&buf[..n]);

    let code = request
        .lines()
        .next()
        .and_then(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let uri = parts[1];
                if let Some(query_start) = uri.find('?') {
                    let query = &uri[query_start + 1..];
                    for pair in query.split('&') {
                        let mut kv = pair.splitn(2, '=');
                        let key = kv.next().unwrap_or("");
                        let value = kv.next().unwrap_or("");
                        if key == "code" {
                            return Some(
                                urlencoding::decode(value)
                                    .ok()
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| value.to_string()),
                            );
                        }
                    }
                }
            }
            None
        })
        .ok_or_else(|| "No auth code in callback".to_string())?;

    let response = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Type: text/html\r\n",
        "Connection: close\r\n",
        "\r\n",
        "<!DOCTYPE html><html><body style='background:#1a1a2e;color:#e0e0e0;",
        "display:flex;align-items:center;justify-content:center;height:100vh;",
        "font-family:sans-serif;'><div style='text-align:center;'>",
        "<h2>Authorization Successful</h2>",
        "<p>You can close this window and return to the terminal.</p>",
        "</div></body></html>"
    );
    tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes())
        .await
        .ok();

    Ok(code)
}

pub struct AnthropicOAuth;

#[async_trait::async_trait]
impl OAuthProvider for AnthropicOAuth {
    fn id(&self) -> &str {
        "anthropic"
    }
    fn name(&self) -> &str {
        "Anthropic"
    }

    async fn login(&self, callbacks: &OAuthLoginCallbacks) -> Result<OAuthCredentials, String> {
        let (verifier, challenge) = generate_pkce();

        let auth_url = format!(
            "https://claude.ai/oauth/authorize?response_type=code&client_id={}&redirect_uri={}&code_challenge={}&code_challenge_method=S256&scope={}",
            "9d1c250a-e61b-44d9-88ed-5944d1962f5e",
            urlencoding::encode("http://localhost:53692/callback"),
            &challenge,
            urlencoding::encode(
                "org:create_api_key user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload"
            ),
        );

        (callbacks.on_auth_url)(&auth_url);

        let code_result = start_callback_server(53692, "callback", 300).await;

        let code = match code_result {
            Ok(code) => code,
            Err(_) => {
                (callbacks.on_prompt)("Please enter the authorization code from your browser:")
                    .ok_or_else(|| "No authorization code provided".to_string())?
            }
        };

        let client = reqwest::Client::new();
        let params = [
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", "http://localhost:53692/callback"),
            ("client_id", "9d1c250a-e61b-44d9-88ed-5944d1962f5e"),
            ("code_verifier", &verifier),
        ];

        let resp = client
            .post("https://platform.claude.com/v1/oauth/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Token exchange failed: {}", e))?;

        let text = resp.text().await.unwrap_or_default();
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse token response: {}: {}", e, text))?;

        let access_token = json
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Missing access_token: {}", text))?
            .to_string();
        let refresh_token = json
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Missing refresh_token: {}", text))?
            .to_string();
        let expires_in = json
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600);

        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
            + expires_in as i64;

        Ok(OAuthCredentials {
            refresh_token,
            access_token,
            expires_at,
            extra: HashMap::new(),
        })
    }

    async fn refresh_token(
        &self,
        credentials: &OAuthCredentials,
    ) -> Result<OAuthCredentials, String> {
        let client = reqwest::Client::new();
        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", &credentials.refresh_token),
            ("client_id", "9d1c250a-e61b-44d9-88ed-5944d1962f5e"),
        ];

        let resp = client
            .post("https://platform.claude.com/v1/oauth/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Token refresh failed: {}", e))?;

        let text = resp.text().await.unwrap_or_default();
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse refresh response: {}: {}", e, text))?;

        let access_token = json
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Missing access_token: {}", text))?
            .to_string();
        let refresh_token = json
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .unwrap_or(&credentials.refresh_token)
            .to_string();
        let expires_in = json
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600);

        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
            + expires_in as i64;

        Ok(OAuthCredentials {
            refresh_token,
            access_token,
            expires_at,
            extra: HashMap::new(),
        })
    }

    fn get_api_key(&self, credentials: &OAuthCredentials) -> Option<String> {
        Some(credentials.access_token.clone())
    }
}

pub struct GitHubCopilotOAuth;

#[async_trait::async_trait]
impl OAuthProvider for GitHubCopilotOAuth {
    fn id(&self) -> &str {
        "github-copilot"
    }
    fn name(&self) -> &str {
        "GitHub Copilot"
    }

    async fn login(&self, callbacks: &OAuthLoginCallbacks) -> Result<OAuthCredentials, String> {
        let enterprise_domain = (callbacks.on_prompt)(
            "GitHub Copilot enterprise domain (press Enter for default github.com):",
        )
        .unwrap_or_default();

        let domain = if enterprise_domain.is_empty() {
            "github.com".to_string()
        } else {
            enterprise_domain
        };

        let client_id = "Iv1.b507a08c87ecfe98";
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("https://{}/login/device/code", domain))
            .header("Accept", "application/json")
            .form(&[("client_id", client_id), ("scope", "read:user")])
            .send()
            .await
            .map_err(|e| format!("Device code request failed: {}", e))?;

        let text = resp.text().await.unwrap_or_default();
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("Parse error: {}: {}", e, text))?;

        let device_code = json
            .get("device_code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Missing device_code: {}", text))?
            .to_string();
        let user_code = json
            .get("user_code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Missing user_code: {}", text))?
            .to_string();
        let verification_uri = json
            .get("verification_uri")
            .and_then(|v| v.as_str())
            .unwrap_or("https://github.com/login/device")
            .to_string();
        let interval = json.get("interval").and_then(|v| v.as_u64()).unwrap_or(5);
        let expires_in = json
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(900);

        (callbacks.on_device_code)(&user_code, &verification_uri);

        let token_url = format!("https://{}/login/oauth/access_token", domain);
        let result = poll_device_code_flow(
            &device_code,
            interval,
            expires_in,
            &token_url,
            client_id,
            None,
            callbacks.signal.clone(),
        )
        .await?;

        let copilot_resp = client
            .get("https://api.github.com/copilot_internal/v2/token")
            .header("Authorization", format!("Bearer {}", result.access_token))
            .header("User-Agent", "GitHubCopilotChat/0.35.0")
            .send()
            .await
            .map_err(|e| format!("Copilot token request failed: {}", e))?;

        let copilot_text = copilot_resp.text().await.unwrap_or_default();
        let copilot_json: serde_json::Value = serde_json::from_str(&copilot_text)
            .map_err(|e| format!("Parse copilot token response: {}: {}", e, copilot_text))?;

        let token = copilot_json
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Missing copilot token: {}", copilot_text))?;
        let refresh_in = copilot_json
            .get("refresh_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(1500);

        let base_url = extract_copilot_base_url(token);

        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
            + refresh_in as i64;

        let mut extra = HashMap::new();
        extra.insert(
            "enterprise_domain".to_string(),
            serde_json::Value::String(domain),
        );
        extra.insert("base_url".to_string(), serde_json::Value::String(base_url));

        Ok(OAuthCredentials {
            refresh_token: result.access_token,
            access_token: token.to_string(),
            expires_at,
            extra,
        })
    }

    async fn refresh_token(
        &self,
        credentials: &OAuthCredentials,
    ) -> Result<OAuthCredentials, String> {
        let enterprise_domain = credentials
            .extra
            .get("enterprise_domain")
            .and_then(|v| v.as_str())
            .unwrap_or("github.com");

        let client = reqwest::Client::new();

        let resp = client
            .post(format!(
                "https://{}/login/oauth/access_token",
                enterprise_domain
            ))
            .header("Accept", "application/json")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", &credentials.refresh_token),
                ("client_id", "Iv1.b507a08c87ecfe98"),
            ])
            .send()
            .await
            .map_err(|e| format!("GitHub token refresh failed: {}", e))?;

        let text = resp.text().await.unwrap_or_default();
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("Parse error: {}: {}", e, text))?;

        let access_token = json
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Missing access_token: {}", text))?
            .to_string();
        let new_refresh_token = json
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .unwrap_or(&credentials.refresh_token)
            .to_string();

        let copilot_resp = client
            .get("https://api.github.com/copilot_internal/v2/token")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "GitHubCopilotChat/0.35.0")
            .send()
            .await
            .map_err(|e| format!("Copilot token refresh failed: {}", e))?;

        let copilot_text = copilot_resp.text().await.unwrap_or_default();
        let copilot_json: serde_json::Value = serde_json::from_str(&copilot_text)
            .map_err(|e| format!("Parse copilot token: {}: {}", e, copilot_text))?;

        let token = copilot_json
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Missing copilot token: {}", copilot_text))?;
        let refresh_in = copilot_json
            .get("refresh_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(1500);

        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
            + refresh_in as i64;

        let mut extra = credentials.extra.clone();
        let base_url = extract_copilot_base_url(token);
        extra.insert("base_url".to_string(), serde_json::Value::String(base_url));

        Ok(OAuthCredentials {
            refresh_token: new_refresh_token,
            access_token: token.to_string(),
            expires_at,
            extra,
        })
    }

    fn get_api_key(&self, credentials: &OAuthCredentials) -> Option<String> {
        Some(credentials.access_token.clone())
    }
}

fn extract_copilot_base_url(token: &str) -> String {
    let parts: Vec<&str> = token.splitn(3, '.').collect();
    if parts.len() >= 2 {
        use base64::Engine;
        let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        if let Ok(decoded) = engine.decode(parts[1]) {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&decoded) {
                if let Some(proxy_ep) = json.get("proxy_ep").and_then(|v| v.as_str()) {
                    return format!("https://{}/v1", proxy_ep);
                }
            }
        }
    }
    String::new()
}

pub struct OpenAICodexOAuth;

#[async_trait::async_trait]
impl OAuthProvider for OpenAICodexOAuth {
    fn id(&self) -> &str {
        "openai-codex"
    }
    fn name(&self) -> &str {
        "OpenAI Codex"
    }

    async fn login(&self, callbacks: &OAuthLoginCallbacks) -> Result<OAuthCredentials, String> {
        let (verifier, challenge) = generate_pkce();

        let auth_url = format!(
            "https://auth.openai.com/oauth/authorize?response_type=code&client_id={}&redirect_uri={}&code_challenge={}&code_challenge_method=S256&scope={}",
            "app_EMoamEEZ73f0CkXaXp7hrann",
            urlencoding::encode("http://localhost:1455/auth/callback"),
            &challenge,
            urlencoding::encode("openid profile email offline_access"),
        );

        (callbacks.on_auth_url)(&auth_url);

        let code_result = start_callback_server(1455, "auth/callback", 300).await;

        let code = match code_result {
            Ok(code) => code,
            Err(_) => {
                (callbacks.on_prompt)("Please enter the authorization code from your browser:")
                    .ok_or_else(|| "No authorization code provided".to_string())?
            }
        };

        let client = reqwest::Client::new();
        let params = [
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", "http://localhost:1455/auth/callback"),
            ("client_id", "app_EMoamEEZ73f0CkXaXp7hrann"),
            ("code_verifier", &verifier),
        ];

        let resp = client
            .post("https://auth.openai.com/oauth/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Token exchange failed: {}", e))?;

        let text = resp.text().await.unwrap_or_default();
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse token response: {}: {}", e, text))?;

        let access_token = json
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Missing access_token: {}", text))?
            .to_string();
        let refresh_token = json
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Missing refresh_token: {}", text))?
            .to_string();
        let expires_in = json
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600);

        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
            + expires_in as i64;

        let account_id = extract_openai_account_id(&access_token);

        let mut extra = HashMap::new();
        if let Some(aid) = account_id {
            extra.insert("account_id".to_string(), serde_json::Value::String(aid));
        }

        Ok(OAuthCredentials {
            refresh_token,
            access_token,
            expires_at,
            extra,
        })
    }

    async fn refresh_token(
        &self,
        credentials: &OAuthCredentials,
    ) -> Result<OAuthCredentials, String> {
        let client = reqwest::Client::new();
        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", &credentials.refresh_token),
            ("client_id", "app_EMoamEEZ73f0CkXaXp7hrann"),
        ];

        let resp = client
            .post("https://auth.openai.com/oauth/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Token refresh failed: {}", e))?;

        let text = resp.text().await.unwrap_or_default();
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse refresh: {}: {}", e, text))?;

        let access_token = json
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Missing access_token: {}", text))?
            .to_string();
        let refresh_token = json
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .unwrap_or(&credentials.refresh_token)
            .to_string();
        let expires_in = json
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600);

        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
            + expires_in as i64;

        let mut extra = credentials.extra.clone();
        let account_id = extract_openai_account_id(&access_token);
        if let Some(aid) = account_id {
            extra.insert("account_id".to_string(), serde_json::Value::String(aid));
        }

        Ok(OAuthCredentials {
            refresh_token,
            access_token,
            expires_at,
            extra,
        })
    }

    fn get_api_key(&self, credentials: &OAuthCredentials) -> Option<String> {
        Some(credentials.access_token.clone())
    }
}

fn extract_openai_account_id(token: &str) -> Option<String> {
    let parts: Vec<&str> = token.splitn(3, '.').collect();
    if parts.len() >= 2 {
        use base64::Engine;
        let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        if let Ok(decoded) = engine.decode(parts[1]) {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&decoded) {
                return json
                    .pointer("/https://api.openai.com/auth")
                    .and_then(|v| v.get("account_id"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pkce_generation() {
        let (verifier, challenge) = generate_pkce();
        assert!(!verifier.is_empty());
        assert!(!challenge.is_empty());
        assert_ne!(verifier, challenge);
    }

    #[test]
    fn test_base64_url_encode() {
        let encoded = base64_url_encode(b"hello");
        assert!(!encoded.contains('+'));
        assert!(!encoded.contains('/'));
        assert!(!encoded.contains('='));
    }

    #[test]
    fn test_extract_copilot_base_url_empty() {
        let url = extract_copilot_base_url("invalid-token");
        assert_eq!(url, "");
    }

    #[test]
    fn test_oauth_registry() {
        let providers = list_oauth_providers();
        assert!(providers.contains(&"anthropic".to_string()));
        assert!(providers.contains(&"github-copilot".to_string()));
        assert!(providers.contains(&"openai-codex".to_string()));
    }
}
