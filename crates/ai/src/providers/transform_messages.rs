//! Message transformation utilities for cross-provider compatibility

use std::collections::HashMap;

use crate::types::{AssistantMessage, ContentBlock, Message, Model, ToolCall, ToolResultMessage};

const NON_VISION_USER_IMAGE_PLACEHOLDER: &str = "(image omitted: model does not support images)";
const NON_VISION_TOOL_IMAGE_PLACEHOLDER: &str =
    "(tool image omitted: model does not support images)";

/// Replace image content blocks with a text placeholder, avoiding consecutive placeholders.
fn replace_images_with_placeholder(
    content: &[ContentBlock],
    placeholder: &str,
) -> Vec<ContentBlock> {
    let mut result = Vec::new();
    let mut previous_was_placeholder = false;

    for block in content {
        if matches!(block, ContentBlock::Image(_)) {
            if !previous_was_placeholder {
                result.push(ContentBlock::text(placeholder));
            }
            previous_was_placeholder = true;
            continue;
        }

        let is_placeholder = match block {
            ContentBlock::Text(t) => t.text == placeholder,
            _ => false,
        };
        result.push(block.clone());
        previous_was_placeholder = is_placeholder;
    }

    result
}

/// Remove images from messages if the model doesn't support them.
fn downgrade_unsupported_images(messages: &[Message], model: &Model) -> Vec<Message> {
    let supports_images = model
        .input_capabilities
        .iter()
        .any(|c| matches!(c, crate::types::Capability::Image));

    if supports_images {
        return messages.to_vec();
    }

    messages
        .iter()
        .map(|msg| match msg {
            Message::User(u) => {
                let new_content =
                    replace_images_with_placeholder(&u.content, NON_VISION_USER_IMAGE_PLACEHOLDER);
                Message::User(crate::types::UserMessage {
                    content: new_content,
                    ..u.clone()
                })
            }
            Message::ToolResult(tr) => {
                let new_content =
                    replace_images_with_placeholder(&tr.content, NON_VISION_TOOL_IMAGE_PLACEHOLDER);
                Message::ToolResult(ToolResultMessage {
                    content: new_content,
                    ..tr.clone()
                })
            }
            other => other.clone(),
        })
        .collect()
}

/// Transform messages for cross-provider compatibility.
///
/// Performs:
/// - Image downgrade for non-vision models
/// - Tool call ID normalization across providers
/// - Thinking block handling (redacted removal, cross-model conversion)
/// - Synthetic tool result insertion for orphaned tool calls
/// - Errored/aborted assistant message removal
pub fn transform_messages(
    messages: &[Message],
    model: &Model,
    normalize_tool_call_id: Option<&dyn Fn(&str, &Model, &AssistantMessage) -> String>,
) -> Vec<Message> {
    let mut tool_call_id_map: HashMap<String, String> = HashMap::new();
    let image_aware_messages = downgrade_unsupported_images(messages, model);

    // First pass: transform messages
    let transformed: Vec<Message> = image_aware_messages
        .iter()
        .map(|msg| {
            match msg {
                Message::User(_) => msg.clone(),
                Message::ToolResult(tr) => {
                    let normalized_id = tool_call_id_map.get(&tr.tool_call_id);
                    if let Some(nid) = normalized_id
                        && nid != &tr.tool_call_id
                    {
                        let mut new_tr = tr.clone();
                        new_tr.tool_call_id = nid.clone();
                        return Message::ToolResult(new_tr);
                    }
                    msg.clone()
                }
                Message::Assistant(a) => {
                    let is_same_model = a.provider == model.provider.as_str()
                        && a.api == model.api.as_str()
                        && a.model == model.id;

                    let transformed_content: Vec<ContentBlock> = a
                        .content
                        .iter()
                        .flat_map(|block| {
                            match block {
                                ContentBlock::Thinking(tc) => {
                                    if tc.redacted {
                                        // Redacted thinking is opaque, only valid for same model
                                        return if is_same_model {
                                            vec![block.clone()]
                                        } else {
                                            vec![]
                                        };
                                    }
                                    // For same model: keep thinking blocks with signatures
                                    if is_same_model && tc.thinking_signature.is_some() {
                                        return vec![block.clone()];
                                    }
                                    // Skip empty thinking
                                    if tc.thinking.trim().is_empty() {
                                        return vec![];
                                    }
                                    if is_same_model {
                                        return vec![block.clone()];
                                    }
                                    // Cross-model: convert thinking to text
                                    vec![ContentBlock::text(&tc.thinking)]
                                }
                                ContentBlock::Text(tc) => {
                                    if is_same_model {
                                        vec![block.clone()]
                                    } else {
                                        vec![ContentBlock::text(&tc.text)]
                                    }
                                }
                                ContentBlock::ToolCall(tc) => {
                                    let mut normalized = tc.clone();
                                    if !is_same_model && tc.thought_signature.is_some() {
                                        normalized.thought_signature = None;
                                    }
                                    if !is_same_model
                                        && let Some(normalize_fn) = &normalize_tool_call_id
                                    {
                                        let new_id = normalize_fn(&tc.id, model, a);
                                        if new_id != tc.id {
                                            tool_call_id_map.insert(tc.id.clone(), new_id.clone());
                                            normalized.id = new_id;
                                        }
                                    }
                                    vec![ContentBlock::ToolCall(normalized)]
                                }
                                _ => vec![block.clone()],
                            }
                        })
                        .collect();

                    let mut new_a = a.clone();
                    new_a.content = transformed_content;
                    Message::Assistant(new_a)
                }
            }
        })
        .collect();

    // Second pass: insert synthetic tool results for orphaned tool calls
    let mut result: Vec<Message> = Vec::new();
    let mut pending_tool_calls: Vec<ToolCall> = Vec::new();
    let mut existing_tool_result_ids: HashMap<String, bool> = HashMap::new();

    let insert_synthetic = |result: &mut Vec<Message>,
                            pending: &mut Vec<ToolCall>,
                            existing: &mut HashMap<String, bool>| {
        if !pending.is_empty() {
            for tc in pending.drain(..) {
                if !existing.contains_key(&tc.id) {
                    result.push(Message::ToolResult(ToolResultMessage::new(
                        tc.id.clone(),
                        tc.name,
                        vec![ContentBlock::text("No result provided")],
                        true,
                    )));
                }
            }
            existing.clear();
        }
    };

    for msg in &transformed {
        match msg {
            Message::Assistant(a) => {
                insert_synthetic(
                    &mut result,
                    &mut pending_tool_calls,
                    &mut existing_tool_result_ids,
                );

                // Skip errored/aborted assistant messages
                if a.stop_reason == crate::types::StopReason::Error
                    || a.stop_reason == crate::types::StopReason::Aborted
                {
                    continue;
                }

                // Track tool calls
                let tool_calls: Vec<ToolCall> = a
                    .content
                    .iter()
                    .filter_map(|b| {
                        if let ContentBlock::ToolCall(tc) = b {
                            Some(tc.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                if !tool_calls.is_empty() {
                    pending_tool_calls = tool_calls;
                    existing_tool_result_ids.clear();
                }

                result.push(msg.clone());
            }
            Message::ToolResult(tr) => {
                existing_tool_result_ids.insert(tr.tool_call_id.clone(), true);
                result.push(msg.clone());
            }
            Message::User(_) => {
                insert_synthetic(
                    &mut result,
                    &mut pending_tool_calls,
                    &mut existing_tool_result_ids,
                );
                result.push(msg.clone());
            }
        }
    }

    // Handle unresolved tool calls at end
    insert_synthetic(
        &mut result,
        &mut pending_tool_calls,
        &mut existing_tool_result_ids,
    );

    result
}
