//! Image generation types and providers

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock, RwLock};

use serde::{Deserialize, Serialize};

/// Known image API identifiers
pub const KNOWN_IMAGES_API_OPENROUTER: &str = "openrouter-images";

/// Information about an HTTP response (for the onResponse callback).
#[derive(Debug, Clone)]
pub struct ProviderResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
}

/// Options for image generation requests.
#[derive(Clone)]
pub struct ImagesOptions {
    /// API key override (falls back to env if not set)
    pub api_key: Option<String>,
    /// Optional callback for inspecting or replacing provider payloads before sending.
    pub on_payload: Option<
        Arc<dyn Fn(&serde_json::Value, &ImagesModel) -> Option<serde_json::Value> + Send + Sync>,
    >,
    /// Optional callback invoked after an HTTP response is received.
    pub on_response: Option<Arc<dyn Fn(&ProviderResponse, &ImagesModel) + Send + Sync>>,
    /// Optional custom HTTP headers to include in API requests.
    pub headers: Option<HashMap<String, String>>,
    /// HTTP request timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Maximum retry attempts.
    pub max_retries: Option<u32>,
}

impl Default for ImagesOptions {
    fn default() -> Self {
        Self {
            api_key: None,
            on_payload: None,
            on_response: None,
            headers: None,
            timeout_ms: None,
            max_retries: None,
        }
    }
}

/// Image generation provider function signature (async).
/// Takes owned values to allow the returned future to be 'static/'Send.
pub type ImagesFunction = Box<
    dyn Fn(
            ImagesModel,
            ImagesContext,
            Option<ImagesOptions>,
        ) -> Pin<Box<dyn Future<Output = Result<AssistantImages, String>> + Send>>
        + Send
        + Sync,
>;

/// A registered image API provider.
pub struct ImagesApiProvider {
    pub api: String,
    pub generate_images: ImagesFunction,
}

/// Image generation request context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagesContext {
    pub input: Vec<ImagesInputContent>,
}

/// Image generation input content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ImagesInputContent {
    #[serde(rename = "text")]
    Text(ImagesTextContent),
    #[serde(rename = "image")]
    Image(ImagesImageContent),
}

/// Text content for image generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagesTextContent {
    pub text: String,
}

/// Image content reference for image generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagesImageContent {
    pub data: String,
    pub mime_type: String,
}

/// Image generation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantImages {
    pub api: String,
    pub provider: String,
    pub model: String,
    pub output: Vec<ImagesOutputContent>,
    pub response_id: Option<String>,
    pub usage: Option<crate::types::Usage>,
    pub stop_reason: ImagesStopReason,
    pub error_message: Option<String>,
    pub timestamp: i64,
}

/// Image generation output content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ImagesOutputContent {
    #[serde(rename = "text")]
    Text(ImagesTextContent),
    #[serde(rename = "image")]
    Image(ImagesImageContent),
}

/// Why image generation stopped
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ImagesStopReason {
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "aborted")]
    Aborted,
}

/// Image model definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagesModel {
    pub id: String,
    pub name: String,
    pub api: String,
    pub provider: String,
    pub base_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_capabilities: Option<Vec<String>>,
    pub output_capabilities: Vec<String>,
    pub cost: crate::types::ModelCost,
    pub headers: Option<HashMap<String, String>>,
}

// ── Image API Registry ──

static IMAGES_API_REGISTRY: std::sync::LazyLock<RwLock<HashMap<String, ImagesApiProvider>>> =
    std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));

/// Register an image API provider.
pub fn register_images_api_provider(provider: ImagesApiProvider) {
    if let Ok(mut registry) = IMAGES_API_REGISTRY.write() {
        registry.insert(provider.api.clone(), provider);
    }
}

/// Auto-register built-in image providers.
/// Uses OnceLock for safe one-time initialization.
static BUILTIN_IMAGE_PROVIDERS: OnceLock<()> = OnceLock::new();

/// Ensure all built-in image providers are registered.
/// Called automatically on first use of `generate_images()`.
fn ensure_image_providers() {
    BUILTIN_IMAGE_PROVIDERS.get_or_init(|| {
        crate::images_openrouter::register();
    });
}

/// Generate images using a registered provider.
pub async fn generate_images(
    model: &ImagesModel,
    context: &ImagesContext,
    options: Option<&ImagesOptions>,
) -> Result<AssistantImages, String> {
    ensure_image_providers();
    let registry = IMAGES_API_REGISTRY
        .read()
        .map_err(|e| format!("Registry lock error: {}", e))?;
    let provider = registry
        .get(&model.api)
        .ok_or_else(|| format!("No provider registered for API: {}", model.api))?;
    (provider.generate_images)(model.clone(), context.clone(), options.cloned()).await
}
