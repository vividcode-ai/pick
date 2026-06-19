//! OpenAI Responses API shared utilities

use crate::types::ToolDefinition;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Text signature version 1 payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSignatureV1 {
    pub v: u8,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
}

/// Encode a text signature v1 as a JSON string.
pub fn encode_text_signature_v1(id: &str, phase: Option<&str>) -> String {
    let payload = TextSignatureV1 {
        v: 1,
        id: id.to_string(),
        phase: phase.map(|p| p.to_string()),
    };
    serde_json::to_string(&payload).unwrap_or_default()
}

/// Parse a text signature string.
pub fn parse_text_signature(signature: Option<&str>) -> Option<TextSignatureV1> {
    let signature = signature?;
    if signature.starts_with('{')
        && let Ok(parsed) = serde_json::from_str::<TextSignatureV1>(signature)
            && parsed.v == 1 {
                return Some(parsed);
            }
    Some(TextSignatureV1 {
        v: 1,
        id: signature.to_string(),
        phase: None,
    })
}

/// Convert tool definitions to OpenAI Responses API format.
pub fn convert_responses_tools(tools: &[ToolDefinition], strict: Option<bool>) -> Vec<Value> {
    let strict = strict.unwrap_or(false);
    tools
        .iter()
        .map(|tool| {
            serde_json::json!({
                "type": "function",
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters,
                "strict": strict,
            })
        })
        .collect()
}

/// Normalize a tool call ID part for the Responses API (max 64 chars, alphanumeric/underscore/hyphen).
pub fn normalize_id_part(part: &str) -> String {
    let sanitized: String = part
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let truncated: String = sanitized.chars().take(64).collect();
    truncated.trim_end_matches('_').to_string()
}

/// Build a foreign Responses item ID using a hash of the original ID.
pub fn build_foreign_responses_item_id(item_id: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(item_id.as_bytes());
    let hex = format!("{:x}", hash);
    let id = format!("fc_{}", hex);
    let truncated: String = id.chars().take(64).collect();
    truncated
}
