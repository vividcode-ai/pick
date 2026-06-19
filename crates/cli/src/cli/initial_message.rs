//! Build initial message for non-interactive mode

use crate::args::Args;

/// Input for building initial message
pub struct InitialMessageInput {
    pub parsed: Args,
    pub file_text: Option<String>,
    pub file_images: Option<Vec<serde_json::Value>>,
    pub stdin_content: Option<String>,
}

/// Result of building initial message
pub struct InitialMessageResult {
    pub initial_message: Option<String>,
    pub initial_images: Option<Vec<serde_json::Value>>,
}

/// Combine stdin content, @file text, and the first CLI message into a single
/// initial prompt for non-interactive mode.
pub fn build_initial_message(input: InitialMessageInput) -> InitialMessageResult {
    let mut parts: Vec<String> = Vec::new();

    if let Some(stdin) = input.stdin_content
        && !stdin.is_empty()
    {
        parts.push(stdin);
    }

    if let Some(file_text) = input.file_text
        && !file_text.is_empty()
    {
        parts.push(file_text);
    }

    // Consume the first message from parsed args
    let mut parsed = input.parsed;
    if !parsed.messages.is_empty() {
        parts.push(parsed.messages.remove(0));
    }

    InitialMessageResult {
        initial_message: if parts.is_empty() {
            None
        } else {
            Some(parts.join(""))
        },
        initial_images: input.file_images.filter(|v| !v.is_empty()),
    }
}
