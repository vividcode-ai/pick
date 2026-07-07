//! Utilities for flattening developer messages into the system prompt
//! for providers that do not support a separate developer role.

/// Flatten developer_messages into the system prompt string.
/// Each developer message is wrapped in an XML-style marker tag and appended
/// to the existing system prompt text.
///
/// If `system_prompt` is `None`, returns `None`.
/// If `developer_messages` is empty, returns `system_prompt` unchanged.
pub fn flatten_developer_messages(
    system_prompt: Option<String>,
    developer_messages: &[String],
) -> Option<String> {
    let base = match system_prompt {
        Some(s) => s,
        None => return None,
    };

    if developer_messages.is_empty() {
        return Some(base);
    }

    let mut result = base;
    for msg in developer_messages {
        result.push_str("\n\n");
        result.push_str(msg);
    }
    Some(result)
}
