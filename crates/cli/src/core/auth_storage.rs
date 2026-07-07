//! Credential storage for API keys and OAuth tokens

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::core::resolve_config_value::resolve_config_value;
pub use pick_agent::auth::{AuthCredential, AuthStorageData, OAuthCredentials, default_auth_path};

#[derive(Debug, Clone)]
pub struct AuthStatus {
    pub configured: bool,
    pub source: Option<String>,
    pub label: Option<String>,
}

// ============================================================================
// LockResult
// ============================================================================

struct LockResult<T> {
    result: T,
    next: Option<String>,
}

// ============================================================================
// StorageBackend enum — replaces trait object for dyn compatibility
// ============================================================================

pub(crate) enum StorageBackend {
    File(FileBackend),
    InMemory(InMemoryBackend),
}

pub(crate) struct FileBackend {
    auth_path: PathBuf,
    lock: Mutex<()>,
}

impl FileBackend {
    fn new(auth_path: Option<PathBuf>) -> Self {
        Self {
            auth_path: auth_path.unwrap_or_else(default_auth_path),
            lock: Mutex::new(()),
        }
    }

    fn ensure_dir(&self) {
        if let Some(parent) = self.auth_path.parent() {
            fs::create_dir_all(parent).ok();
        }
    }

    fn ensure_file(&self) {
        if !self.auth_path.exists() {
            if let Some(parent) = self.auth_path.parent() {
                fs::create_dir_all(parent).ok();
            }
            fs::write(&self.auth_path, "{}").ok();
        }
    }

    fn read_content(&self) -> Option<String> {
        fs::read_to_string(&self.auth_path).ok()
    }

    fn write_content(&self, content: &str) {
        // Use cross-process file locking via fs2 to prevent concurrent writes
        if let Ok(file) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.auth_path)
        {
            use fs2::FileExt;
            let _ = file.lock_exclusive();
            use std::io::Write;
            let mut file = file;
            let _ = file.write_all(content.as_bytes());
            // Lock released when file handle is dropped
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.auth_path, fs::Permissions::from_mode(0o600)).ok();
        }
    }
}

pub(crate) struct InMemoryBackend {
    value: Mutex<Option<String>>,
}

impl InMemoryBackend {
    fn new() -> Self {
        Self {
            value: Mutex::new(None),
        }
    }
}

impl StorageBackend {
    fn with_lock<T, F>(&self, f: F) -> T
    where
        F: FnOnce(Option<&str>) -> LockResult<T>,
    {
        match self {
            StorageBackend::File(b) => {
                let _guard = b.lock.lock().unwrap();
                b.ensure_dir();
                b.ensure_file();
                let current = b.read_content();
                let result = f(current.as_deref());
                if let Some(next) = result.next {
                    b.write_content(&next);
                }
                result.result
            }
            StorageBackend::InMemory(b) => {
                let mut value = b.value.lock().unwrap();
                let result = f(value.as_deref());
                if let Some(next) = result.next {
                    *value = Some(next);
                }
                result.result
            }
        }
    }

    fn with_lock_async<T, F, Fut>(&self, f: F) -> T
    where
        F: FnOnce(Option<&str>) -> Fut,
        Fut: std::future::Future<Output = LockResult<T>>,
    {
        let rt = tokio::runtime::Handle::try_current()
            .expect("Need a tokio runtime for with_lock_async");
        match self {
            StorageBackend::File(b) => {
                let _guard = b.lock.lock().unwrap();
                b.ensure_dir();
                b.ensure_file();
                let current = b.read_content();
                let result = rt.block_on(f(current.as_deref()));
                if let Some(next) = result.next {
                    b.write_content(&next);
                }
                result.result
            }
            StorageBackend::InMemory(b) => {
                let mut value = b.value.lock().unwrap();
                let result = rt.block_on(f(value.as_deref()));
                if let Some(next) = result.next {
                    *value = Some(next);
                }
                result.result
            }
        }
    }
}

// ============================================================================
// AuthStorage
// ============================================================================

pub struct AuthStorage {
    data: Mutex<AuthStorageData>,
    runtime_overrides: Mutex<HashMap<String, String>>,
    fallback_resolver: Mutex<Option<Box<dyn Fn(&str) -> Option<String> + Send>>>,
    load_error: Mutex<Option<String>>,
    errors: Mutex<Vec<String>>,
    storage: StorageBackend,
}

impl AuthStorage {
    pub fn create(auth_path: Option<PathBuf>) -> Self {
        let storage = StorageBackend::File(FileBackend::new(auth_path));
        Self::from_storage(storage)
    }

    pub fn from_storage(storage: StorageBackend) -> Self {
        let instance = Self {
            data: Mutex::new(HashMap::new()),
            runtime_overrides: Mutex::new(HashMap::new()),
            fallback_resolver: Mutex::new(None),
            load_error: Mutex::new(None),
            errors: Mutex::new(Vec::new()),
            storage,
        };
        instance.reload();
        instance
    }

    pub fn in_memory(data: Option<AuthStorageData>) -> Self {
        let data = data.unwrap_or_default();
        let json = serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string());
        let backend = {
            let b = InMemoryBackend::new();
            b.value.lock().unwrap().replace(json);
            b
        };
        Self::from_storage(StorageBackend::InMemory(backend))
    }

    pub fn set_runtime_api_key(&self, provider: &str, api_key: String) {
        self.runtime_overrides
            .lock()
            .unwrap()
            .insert(provider.to_string(), api_key);
    }

    pub fn remove_runtime_api_key(&self, provider: &str) {
        self.runtime_overrides.lock().unwrap().remove(provider);
    }

    pub fn set_fallback_resolver<F>(&self, resolver: F)
    where
        F: Fn(&str) -> Option<String> + Send + 'static,
    {
        *self.fallback_resolver.lock().unwrap() = Some(Box::new(resolver));
    }

    fn record_error(&self, error: impl std::fmt::Display) {
        self.errors.lock().unwrap().push(error.to_string());
    }

    fn parse_storage_data(&self, content: Option<&str>) -> AuthStorageData {
        content
            .and_then(|c| {
                // Accept both old format (pure HashMap) and new format (with last_* fields)
                if let Ok(map) = serde_json::from_str::<HashMap<String, AuthCredential>>(c) {
                    return Some(map);
                }
                // New format: extract only credential entries (those with a "type" field)
                let raw: HashMap<String, serde_json::Value> = serde_json::from_str(c).ok()?;
                let mut data = AuthStorageData::new();
                for (k, v) in raw {
                    if let Ok(cred) = serde_json::from_value::<AuthCredential>(v) {
                        data.insert(k, cred);
                    }
                }
                Some(data)
            })
            .unwrap_or_default()
    }

    pub fn reload(&self) {
        let content = self.storage.with_lock(|current| LockResult {
            result: current.map(|s| s.to_string()),
            next: None,
        });
        let data = self.parse_storage_data(content.as_deref());
        *self.data.lock().unwrap() = data;
        *self.load_error.lock().unwrap() = None;
    }

    fn persist_provider_change(&self, provider: &str, credential: Option<&AuthCredential>) {
        if self.load_error.lock().unwrap().is_some() {
            return;
        }
        self.storage.with_lock(|current| {
            // Parse all fields from the file (credentials + metadata like last_*)
            let raw: HashMap<String, serde_json::Value> = current
                .and_then(|c| serde_json::from_str(c).ok())
                .unwrap_or_default();
            let mut extra = HashMap::new();
            let mut current_data = AuthStorageData::new();
            for (k, v) in &raw {
                if let Ok(cred) = serde_json::from_value::<AuthCredential>(v.clone()) {
                    current_data.insert(k.clone(), cred);
                } else {
                    extra.insert(k.clone(), v.clone());
                }
            }
            match credential {
                Some(cred) => {
                    current_data.insert(provider.to_string(), cred.clone());
                }
                None => {
                    current_data.remove(provider);
                }
            }
            // Rebuild full JSON object preserving extra fields
            let mut out = serde_json::Map::new();
            for (k, v) in &current_data {
                if let Ok(val) = serde_json::to_value(v) {
                    out.insert(k.clone(), val);
                }
            }
            for (k, v) in &extra {
                out.insert(k.clone(), v.clone());
            }
            let json = serde_json::to_string_pretty(&out).unwrap_or_default();
            LockResult {
                result: (),
                next: Some(json),
            }
        });
    }

    pub fn get(&self, provider: &str) -> Option<AuthCredential> {
        self.data.lock().unwrap().get(provider).cloned()
    }

    pub fn set(&self, provider: &str, credential: AuthCredential) {
        self.data
            .lock()
            .unwrap()
            .insert(provider.to_string(), credential.clone());
        self.persist_provider_change(provider, Some(&credential));
    }

    /// Convenience method: set a simple API key credential.
    /// Equivalent to `set(provider, AuthCredential::ApiKey { key: key.into() })`.
    pub fn set_api_key(&self, provider: &str, key: &str) {
        self.set(
            provider,
            AuthCredential::ApiKey {
                key: key.to_string(),
            },
        );
    }

    pub fn remove(&self, provider: &str) {
        self.data.lock().unwrap().remove(provider);
        self.persist_provider_change(provider, None);
    }

    pub fn list(&self) -> Vec<String> {
        self.data.lock().unwrap().keys().cloned().collect()
    }

    /// Alias for `list()`, matching the stub AuthStorage API.
    pub fn list_providers(&self) -> Vec<String> {
        self.list()
    }

    pub fn has(&self, provider: &str) -> bool {
        self.data.lock().unwrap().contains_key(provider)
    }

    pub fn has_auth(&self, provider: &str) -> bool {
        if self
            .runtime_overrides
            .lock()
            .unwrap()
            .contains_key(provider)
        {
            return true;
        }
        if self.data.lock().unwrap().contains_key(provider) {
            return true;
        }
        if find_env_key(provider).is_some() {
            return true;
        }
        if let Some(ref resolver) = *self.fallback_resolver.lock().unwrap()
            && resolver(provider).is_some()
        {
            return true;
        }
        false
    }

    pub fn get_auth_status(&self, provider: &str) -> AuthStatus {
        if self.data.lock().unwrap().contains_key(provider) {
            return AuthStatus {
                configured: true,
                source: Some("stored".to_string()),
                label: None,
            };
        }
        if self
            .runtime_overrides
            .lock()
            .unwrap()
            .contains_key(provider)
        {
            return AuthStatus {
                configured: false,
                source: Some("runtime".to_string()),
                label: Some("--api-key".to_string()),
            };
        }
        if let Some(key) = find_env_key(provider) {
            return AuthStatus {
                configured: false,
                source: Some("environment".to_string()),
                label: Some(key),
            };
        }
        if let Some(ref resolver) = *self.fallback_resolver.lock().unwrap()
            && resolver(provider).is_some()
        {
            return AuthStatus {
                configured: false,
                source: Some("fallback".to_string()),
                label: Some("custom provider config".to_string()),
            };
        }
        AuthStatus {
            configured: false,
            source: None,
            label: None,
        }
    }

    pub fn get_all(&self) -> AuthStorageData {
        self.data.lock().unwrap().clone()
    }

    pub fn drain_errors(&self) -> Vec<String> {
        self.errors.lock().unwrap().drain(..).collect()
    }

    pub async fn get_api_key(&self, provider_id: &str, include_fallback: bool) -> Option<String> {
        // Runtime override takes highest priority
        if let Some(key) = self.runtime_overrides.lock().unwrap().get(provider_id) {
            return Some(key.clone());
        }

        let data = self.data.lock().unwrap();
        let cred = data.get(provider_id);

        match cred {
            Some(AuthCredential::ApiKey { key }) => {
                return resolve_config_value(key);
            }
            Some(AuthCredential::Oauth { inner }) => {
                let stored_creds = inner.clone();
                drop(data);
                return self
                    .refresh_oauth_credentials(provider_id, stored_creds)
                    .await;
            }
            None => {}
        }
        drop(data);

        // Environment variable
        if let Some(env_key) = find_env_key(provider_id) {
            return std::env::var(&env_key).ok().filter(|k| !k.is_empty());
        }

        // Fallback resolver
        if include_fallback && let Some(ref resolver) = *self.fallback_resolver.lock().unwrap() {
            return resolver(provider_id);
        }

        None
    }

    /// Handle OAuth credentials with auto-refresh.
    /// Converts stored credentials to the oauth module format, calls
    /// `get_oauth_api_key()` (which refreshes if expired), persists any
    /// refreshed credentials, and returns the API key.
    async fn refresh_oauth_credentials(
        &self,
        provider_id: &str,
        stored: OAuthCredentials,
    ) -> Option<String> {
        use std::collections::HashMap;

        // Convert stored format (ms) → oauth module format (seconds)
        let mut oauth_creds = pick_ai::oauth::OAuthCredentials {
            access_token: stored.access_token.clone(),
            refresh_token: stored.refresh_token.clone().unwrap_or_default(),
            expires_at: stored.expires / 1000,
            extra: HashMap::new(),
        };

        match pick_ai::oauth::get_oauth_api_key(provider_id, &mut oauth_creds).await {
            Ok(api_key) => {
                // Persist refreshed credentials back to storage
                let new_stored = OAuthCredentials {
                    access_token: oauth_creds.access_token,
                    refresh_token: Some(oauth_creds.refresh_token).filter(|s| !s.is_empty()),
                    expires: oauth_creds.expires_at * 1000,
                    token_type: stored.token_type,
                    scope: stored.scope,
                };
                self.set(provider_id, AuthCredential::Oauth { inner: new_stored });
                Some(api_key)
            }
            Err(_) => None,
        }
    }
}

/// Find the common environment variable key for a provider
fn find_env_key(provider: &str) -> Option<String> {
    let provider_upper = provider.to_uppercase().replace('-', "_");
    let candidates = [
        format!("{}_API_KEY", provider_upper),
        format!("{}_APIKEY", provider_upper),
        format!("{}_KEY", provider_upper),
        format!("{}_TOKEN", provider_upper),
        "ANTHROPIC_API_KEY".to_string(),
        "OPENAI_API_KEY".to_string(),
        "GOOGLE_API_KEY".to_string(),
    ];
    candidates
        .into_iter()
        .find(|key| std::env::var(key).is_ok())
}
