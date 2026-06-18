//! GitHub Copilot-specific HTTP headers

use crate::types::Message;

/// Infer Copilot initiator from the last message role.
/// "agent" if last message is not from user (assistant/tool result), "user" otherwise.
pub fn infer_copilot_initiator(messages: &[Message]) -> &'static str {
    let last = messages.last();
    match last {
        Some(Message::User(_)) => "user",
        _ => "agent",
    }
}

/// Check if any message contains image content (for Copilot-Vision-Request header).
pub fn has_copilot_vision_input(messages: &[Message]) -> bool {
    messages.iter().any(|msg| {
        let content = match msg {
            Message::User(u) => &u.content,
            Message::ToolResult(tr) => &tr.content,
            Message::Assistant(a) => &a.content,
        };
        content.iter().any(|c| matches!(c, crate::types::ContentBlock::Image(_)))
    })
}

/// Build Copilot dynamic headers based on message content.
pub fn build_copilot_dynamic_headers(messages: &[Message], has_images: bool) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    headers.push(("X-Initiator".to_string(), infer_copilot_initiator(messages).to_string()));
    headers.push(("Openai-Intent".to_string(), "conversation-edits".to_string()));
    if has_images {
        headers.push(("Copilot-Vision-Request".to_string(), "true".to_string()));
    }
    headers
}
