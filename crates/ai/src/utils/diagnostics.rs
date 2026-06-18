//! Diagnostic utilities

use serde::{Deserialize, Serialize};

/// A diagnostic message from a provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessageDiagnostic {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}
