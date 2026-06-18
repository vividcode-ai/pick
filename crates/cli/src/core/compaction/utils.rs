use serde_json::Value;
use std::collections::HashSet;

/// File paths touched by a session branch or compaction range
#[derive(Debug, Clone, Default)]
pub struct FileOperations {
    /// Files read but not necessarily modified
    pub read: HashSet<String>,
    /// Files written by full-file write operations
    pub written: HashSet<String>,
    /// Files modified by edit operations
    pub edited: HashSet<String>,
}

/// Create an empty file-operation accumulator
pub fn create_file_ops() -> FileOperations {
    FileOperations::default()
}

/// Add file operations from assistant tool calls to an accumulator
pub fn extract_file_ops_from_message(message: &Value, file_ops: &mut FileOperations) {
    let role = message.get("role").and_then(|v| v.as_str()).unwrap_or("");
    if role != "assistant" {
        return;
    }
    let content = match message.get("content") {
        Some(Value::Array(arr)) => arr,
        _ => return,
    };

    for block in content {
        let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if block_type != "toolCall" {
            continue;
        }
        let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let args = match block.get("arguments") {
            Some(Value::Object(m)) => m,
            _ => continue,
        };
        let path = match args.get("path") {
            Some(Value::String(s)) => s.clone(),
            _ => continue,
        };

        match name {
            "read" => {
                file_ops.read.insert(path);
            }
            "write" => {
                file_ops.written.insert(path);
            }
            "edit" => {
                file_ops.edited.insert(path);
            }
            _ => {}
        }
    }
}

/// Compute sorted read-only and modified file lists from accumulated operations
pub fn compute_file_lists(file_ops: &FileOperations) -> (Vec<String>, Vec<String>) {
    let modified: HashSet<&String> = file_ops.edited.union(&file_ops.written).collect();
    let mut read_files: Vec<String> = file_ops
        .read
        .iter()
        .filter(|f| !modified.contains(*f))
        .cloned()
        .collect();
    read_files.sort();
    let mut modified_files: Vec<String> = modified.into_iter().cloned().collect();
    modified_files.sort();
    (read_files, modified_files)
}

/// Format file lists as summary metadata tags
pub fn format_file_operations(read_files: &[String], modified_files: &[String]) -> String {
    let mut sections = Vec::new();
    if !read_files.is_empty() {
        sections.push(format!(
            "<read-files>\n{}</read-files>",
            read_files.join("\n")
        ));
    }
    if !modified_files.is_empty() {
        sections.push(format!(
            "<modified-files>\n{}</modified-files>",
            modified_files.join("\n")
        ));
    }
    if sections.is_empty() {
        return String::new();
    }
    format!("\n\n{}", sections.join("\n\n"))
}

const TOOL_RESULT_MAX_CHARS: usize = 2000;

fn safe_json_stringify(value: &Value) -> String {
    match serde_json::to_string(value) {
        Ok(s) => s,
        Err(_) => "[unserializable]".to_string(),
    }
}

fn truncate_for_summary(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        return text.to_string();
    }
    let truncated_chars = text.len() - max_chars;
    format!(
        "{}\n\n[... {} more characters truncated]",
        &text[..max_chars],
        truncated_chars
    )
}

/// Serialize LLM messages to plain text for summarization prompts
pub fn serialize_conversation(messages: &[Value]) -> String {
    let mut parts: Vec<String> = Vec::new();

    for msg in messages {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
        match role {
            "user" => {
                let content = msg.get("content");
                let text = match content {
                    Some(Value::String(s)) => s.clone(),
                    Some(Value::Array(arr)) => arr
                        .iter()
                        .filter(|c| c.get("type").and_then(|v| v.as_str()) == Some("text"))
                        .filter_map(|c| c.get("text").and_then(|v| v.as_str()))
                        .collect::<Vec<_>>()
                        .join(""),
                    _ => continue,
                };
                if !text.is_empty() {
                    parts.push(format!("[User]: {}", text));
                }
            }
            "assistant" => {
                let content = match msg.get("content") {
                    Some(Value::Array(arr)) => arr,
                    _ => continue,
                };
                let mut text_parts: Vec<String> = Vec::new();
                let mut thinking_parts: Vec<String> = Vec::new();
                let mut tool_calls: Vec<String> = Vec::new();

                for block in content {
                    let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    match block_type {
                        "text" => {
                            if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                                text_parts.push(t.to_string());
                            }
                        }
                        "thinking" => {
                            if let Some(t) = block.get("thinking").and_then(|v| v.as_str()) {
                                thinking_parts.push(t.to_string());
                            }
                        }
                        "toolCall" => {
                            let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            let args = block.get("arguments").unwrap_or(&Value::Null);
                            let args_str = match args {
                                Value::Object(m) => m
                                    .iter()
                                    .map(|(k, v)| format!("{}={}", k, safe_json_stringify(v)))
                                    .collect::<Vec<_>>()
                                    .join(", "),
                                _ => String::new(),
                            };
                            tool_calls.push(format!("{}({})", name, args_str));
                        }
                        _ => {}
                    }
                }

                if !thinking_parts.is_empty() {
                    parts.push(format!(
                        "[Assistant thinking]: {}",
                        thinking_parts.join("\n")
                    ));
                }
                if !text_parts.is_empty() {
                    parts.push(format!("[Assistant]: {}", text_parts.join("\n")));
                }
                if !tool_calls.is_empty() {
                    parts.push(format!("[Assistant tool calls]: {}", tool_calls.join("; ")));
                }
            }
            "toolResult" => {
                let content = match msg.get("content") {
                    Some(Value::Array(arr)) => arr,
                    _ => continue,
                };
                let text: String = content
                    .iter()
                    .filter(|c| c.get("type").and_then(|v| v.as_str()) == Some("text"))
                    .filter_map(|c| c.get("text").and_then(|v| v.as_str()))
                    .collect();
                if !text.is_empty() {
                    parts.push(format!(
                        "[Tool result]: {}",
                        truncate_for_summary(&text, TOOL_RESULT_MAX_CHARS)
                    ));
                }
            }
            _ => {}
        }
    }

    parts.join("\n\n")
}
