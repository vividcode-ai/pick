//! Context compaction for long sessions

pub mod types;

pub use types::*;

use crate::session::entries::{SessionEntry, SessionEntryKind};
use pick_ai::types::{ContentBlock, Usage};

// ============================================================================
// Token calculation
// ============================================================================

/// Calculate total context tokens from usage
pub fn calculate_context_tokens(usage: &Usage) -> u64 {
    if usage.total_tokens > 0 {
        usage.total_tokens
    } else {
        usage.input + usage.output + usage.cache_read + usage.cache_write
    }
}

/// Get usage from an assistant message entry
fn get_entry_usage(entry: &SessionEntry) -> Option<Usage> {
    if let SessionEntryKind::Message(msg) = &entry.kind
        && msg.role == "assistant"
            && let Some(usage_val) = &msg.usage
                && let Ok(usage) = serde_json::from_value::<Usage>(usage_val.clone()) {
                    return Some(usage);
                }
    None
}

/// Find the last non-aborted assistant message usage from session entries
pub fn get_last_assistant_usage(entries: &[SessionEntry]) -> Option<Usage> {
    for entry in entries.iter().rev() {
        if let SessionEntryKind::Message(msg) = &entry.kind
            && msg.role == "assistant"
                && let Some(usage_val) = &msg.usage
                    && let Ok(usage) = serde_json::from_value::<Usage>(usage_val.clone()) {
                        return Some(usage);
                    }
    }
    None
}

/// Estimate token count for a message using chars/4 heuristic
pub fn estimate_entry_tokens(entry: &SessionEntry) -> u64 {
    let kind = &entry.kind;
    if let SessionEntryKind::Message(msg) = kind {
        match msg.role.as_str() {
            "user" => {
                if let Some(content) = msg.content.as_str() {
                    ceil_div(content.len() as u64, 4)
                } else if let Some(arr) = msg.content.as_array() {
                    let chars: usize = arr
                        .iter()
                        .filter_map(|block| block.get("text").and_then(|t| t.as_str()))
                        .map(|t| t.len())
                        .sum();
                    ceil_div(chars as u64, 4)
                } else {
                    0
                }
            }
            "assistant" => {
                if let Ok(blocks) = serde_json::from_value::<Vec<ContentBlock>>(msg.content.clone())
                {
                    let chars: usize = blocks
                        .iter()
                        .map(|block| match block {
                            ContentBlock::Text(t) => t.text.len(),
                            ContentBlock::Thinking(t) => t.thinking.len(),
                            ContentBlock::ToolCall(t) => {
                                t.name.len() + t.arguments.to_string().len()
                            }
                            ContentBlock::Image(_) => 4800,
                        })
                        .sum();
                    ceil_div(chars as u64, 4)
                } else {
                    0
                }
            }
            "toolResult" => {
                if let Some(text) = msg.content.as_str() {
                    ceil_div(text.len() as u64, 4)
                } else if let Some(arr) = msg.content.as_array() {
                    let chars: usize = arr
                        .iter()
                        .filter_map(|block| block.get("text").and_then(|t| t.as_str()))
                        .map(|t| t.len())
                        .sum();
                    ceil_div(chars as u64, 4)
                } else {
                    0
                }
            }
            _ => 0,
        }
    } else if let SessionEntryKind::Compaction(entry) = kind {
        ceil_div(entry.summary.len() as u64, 4)
    } else if let SessionEntryKind::BranchSummary(entry) = kind {
        ceil_div(entry.summary.len() as u64, 4)
    } else {
        0
    }
}

fn ceil_div(a: u64, b: u64) -> u64 {
    a.div_ceil(b)
}

/// Estimate context tokens from session entries
pub fn estimate_context_tokens(entries: &[SessionEntry]) -> ContextUsageEstimate {
    let usage_info = entries
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, entry)| get_entry_usage(entry).map(|usage| (usage, i)));

    match usage_info {
        Some((usage, last_idx)) => {
            let usage_tokens = calculate_context_tokens(&usage);
            let trailing_tokens: u64 = entries
                .iter()
                .skip(last_idx + 1)
                .map(estimate_entry_tokens)
                .sum();
            ContextUsageEstimate {
                tokens: usage_tokens + trailing_tokens,
                usage_tokens,
                trailing_tokens,
                last_usage_index: Some(last_idx),
            }
        }
        None => {
            let estimated: u64 = entries.iter().map(estimate_entry_tokens).sum();
            ContextUsageEstimate {
                tokens: estimated,
                usage_tokens: 0,
                trailing_tokens: estimated,
                last_usage_index: None,
            }
        }
    }
}

/// Check if compaction should trigger
pub fn should_compact(
    context_tokens: u64,
    context_window: u64,
    settings: &CompactionSettings,
) -> bool {
    if !settings.enabled {
        return false;
    }
    context_tokens > context_window.saturating_sub(settings.reserve_tokens)
}

// ============================================================================
// Cut point detection
// ============================================================================

/// Find valid cut points: indices of entries that can be cut at
pub fn find_valid_cut_points(entries: &[SessionEntry], start: usize, end: usize) -> Vec<usize> {
    let mut cut_points = Vec::new();
    for i in start..end {
        match &entries[i].kind {
            SessionEntryKind::Message(msg) => match msg.role.as_str() {
                "user" | "assistant" | "custom" | "bashExecution" | "branchSummary"
                | "compactionSummary" => {
                    cut_points.push(i);
                }
                _ => {}
            },
            SessionEntryKind::BranchSummary(_) | SessionEntryKind::Custom(_) => {
                cut_points.push(i);
            }
            _ => {}
        }
    }
    cut_points
}

/// Find the user message that starts the turn containing the given entry index
pub fn find_turn_start_index(
    entries: &[SessionEntry],
    entry_index: usize,
    start_index: usize,
) -> isize {
    let mut i = entry_index as isize;
    while i >= start_index as isize {
        match &entries[i as usize].kind {
            SessionEntryKind::Message(msg)
                if (msg.role == "user" || msg.role == "bashExecution") => {
                    return i;
                }
            SessionEntryKind::BranchSummary(_) | SessionEntryKind::Custom(_) => {
                return i;
            }
            _ => {}
        }
        i -= 1;
    }
    -1
}

/// Find the cut point in session entries that keeps approximately `keep_recent_tokens`
pub fn find_cut_point(
    entries: &[SessionEntry],
    start_index: usize,
    end_index: usize,
    keep_recent_tokens: u64,
) -> CutPointResult {
    let cut_points = find_valid_cut_points(entries, start_index, end_index);

    if cut_points.is_empty() {
        return CutPointResult {
            first_kept_entry_index: start_index,
            turn_start_index: -1,
            is_split_turn: false,
        };
    }

    let mut accumulated_tokens: u64 = 0;
    let mut cut_index = cut_points[0];

    for i in (start_index..end_index).rev() {
        let entry = &entries[i];
        if !matches!(entry.kind, SessionEntryKind::Message(_)) {
            continue;
        }

        let message_tokens = estimate_entry_tokens(entry);
        accumulated_tokens += message_tokens;

        if accumulated_tokens >= keep_recent_tokens {
            for &cp in &cut_points {
                if cp >= i {
                    cut_index = cp;
                    break;
                }
            }
            break;
        }
    }

    // Scan backwards to include non-message entries
    while cut_index > start_index {
        let prev_kind = &entries[cut_index - 1].kind;
        match prev_kind {
            SessionEntryKind::Compaction(_) => break,
            SessionEntryKind::Message(_) => break,
            _ => {
                cut_index -= 1;
            }
        }
    }

    // Determine if split turn
    let cut_entry = &entries[cut_index];
    let is_user_message = if let SessionEntryKind::Message(msg) = &cut_entry.kind {
        msg.role == "user"
    } else {
        false
    };
    let turn_start_index = if is_user_message {
        -1
    } else {
        find_turn_start_index(entries, cut_index, start_index)
    };

    CutPointResult {
        first_kept_entry_index: cut_index,
        turn_start_index,
        is_split_turn: !is_user_message && turn_start_index != -1,
    }
}

// ============================================================================
// Compaction preparation
// ============================================================================

/// Extract file operations from messages and previous compaction entries
pub fn extract_file_operations(
    entries: &[SessionEntry],
    _prev_compaction_index: isize,
) -> FileOperations {
    let mut file_ops = FileOperations::new();

    // Extract from tool calls in message entries
    for entry in entries {
        if let SessionEntryKind::Message(msg) = &entry.kind
            && msg.role == "assistant" {
                extract_file_ops_from_content(&msg.content, &mut file_ops);
            }
    }

    file_ops
}

/// Extract file operations from message content (tool calls)
fn extract_file_ops_from_content(content: &serde_json::Value, file_ops: &mut FileOperations) {
    if let Some(blocks) = content.as_array() {
        for block in blocks {
            if let Some(block_type) = block.get("type").and_then(|t| t.as_str())
                && (block_type == "toolCall" || block_type == "tool_use")
                    && let Some(name) = block.get("name").and_then(|n| n.as_str()) {
                        let args = block.get("arguments").or_else(|| block.get("input"));
                        let paths = extract_paths_from_args(args);
                        match name {
                            "Read" | "read" | "Grep" | "grep" => {
                                for p in paths {
                                    if !file_ops.read.contains(&p) {
                                        file_ops.read.push(p);
                                    }
                                }
                            }
                            "Write" | "write" => {
                                for p in paths {
                                    if !file_ops.written.contains(&p) {
                                        file_ops.written.push(p);
                                    }
                                }
                            }
                            "Edit" | "edit" => {
                                for p in paths {
                                    if !file_ops.edited.contains(&p) {
                                        file_ops.edited.push(p);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
        }
    }
}

/// Extract file paths from tool call arguments
fn extract_paths_from_args(args: Option<&serde_json::Value>) -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(args) = args
        && let Some(file_path) = args
            .get("file_path")
            .or_else(|| args.get("path"))
            .and_then(|v| v.as_str())
        {
            paths.push(file_path.to_string());
        }
    paths
}

/// Summarization prompt template
pub const SUMMARIZATION_PROMPT: &str = concat!(
    "The messages above are a conversation to summarize. Create a structured context checkpoint summary ",
    "that another LLM will use to continue the work.\n\n",
    "Use this EXACT format:\n\n",
    "## Goal\n",
    "[What is the user trying to accomplish? Can be multiple items if the session covers different tasks.]\n\n",
    "## Constraints & Preferences\n",
    "- [Any constraints, preferences, or requirements mentioned by user]\n",
    "- [Or '(none)' if none were mentioned]\n\n",
    "## Progress\n",
    "### Done\n",
    "- [x] [Completed tasks/changes]\n\n",
    "### In Progress\n",
    "- [ ] [Current work]\n\n",
    "### Blocked\n",
    "- [Issues preventing progress, if any]\n\n",
    "## Key Decisions\n",
    "- **[Decision]**: [Brief rationale]\n\n",
    "## Next Steps\n",
    "1. [Ordered list of what should happen next]\n\n",
    "## Critical Context\n",
    "- [Any data, examples, or references needed to continue]\n",
    "- [Or '(none)' if not applicable]\n\n",
    "Keep each section concise. Preserve exact file paths, function names, and error messages."
);

/// Update summarization prompt for incremental updates
pub const UPDATE_SUMMARIZATION_PROMPT: &str = concat!(
    "The messages above are NEW conversation messages to incorporate into the existing summary provided ",
    "in <previous-summary> tags.\n\n",
    "Update the existing structured summary with new information. RULES:\n",
    "- PRESERVE all existing information from the previous summary\n",
    "- ADD new progress, decisions, and context from the new messages\n",
    "- UPDATE the Progress section: move items from 'In Progress' to 'Done' when completed\n",
    "- UPDATE 'Next Steps' based on what was accomplished\n",
    "- PRESERVE exact file paths, function names, and error messages\n",
    "- If something is no longer relevant, you may remove it\n\n",
    "Use this EXACT format:\n\n",
    "## Goal"
);

/// Serialize a list of entries to plain text for summarization
pub fn serialize_entries(entries: &[SessionEntry], max_tokens: u64) -> String {
    let mut result = String::new();
    let mut tokens: u64 = 0;

    for entry in entries {
        let text = match &entry.kind {
            SessionEntryKind::Message(msg) => {
                let role = &msg.role;
                let content_text = if let Some(text) = msg.content.as_str() {
                    text.to_string()
                } else if let Some(arr) = msg.content.as_array() {
                    arr.iter()
                        .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    msg.content.to_string()
                };
                format!("[{}]: {}\n", role, content_text)
            }
            SessionEntryKind::Compaction(entry) => {
                format!("[compaction summary]: {}\n", entry.summary)
            }
            _ => continue,
        };

        let entry_tokens = ceil_div(text.len() as u64, 4);
        if tokens + entry_tokens > max_tokens {
            break;
        }
        tokens += entry_tokens;
        result.push_str(&text);
    }

    result
}

/// Trait for generating compaction summaries
#[async_trait::async_trait]
pub trait Summarizer: Send + Sync {
    /// Generate a summary from conversation text
    async fn generate_summary(
        &self,
        conversation_text: &str,
        system_prompt: &str,
    ) -> Result<String, String>;
}

/// Branch summary preamble for when returning from a different branch
pub const BRANCH_SUMMARY_PREAMBLE: &str = "The user explored a different conversation branch before returning here.\nSummary of that exploration:\n\n";

/// Create a branch summary entry from a set of session entries.
/// Uses the provided summarizer to generate a concise summary of the branch conversation.
pub async fn create_branch_summary(
    entries: &[SessionEntry],
    summarizer: &dyn Summarizer,
) -> Result<crate::session::entries::BranchSummaryEntry, String> {
    let text = serialize_entries(entries, 8000);
    let summary = summarizer
        .generate_summary(
            &format!("{}{}", BRANCH_SUMMARY_PREAMBLE, text),
            SUMMARIZATION_PROMPT,
        )
        .await?;

    Ok(crate::session::entries::BranchSummaryEntry { summary })
}

// ============================================================================
// LlmSummarizer — concrete implementation using Pick-ai provider
// ============================================================================

/// An LLM-backed summarizer that uses a registered provider to generate summaries.
pub struct LlmSummarizer {
    model: pick_ai::Model,
    api_key: Option<String>,
    headers: Option<std::collections::HashMap<String, String>>,
    max_tokens: Option<u64>,
}

impl LlmSummarizer {
    pub fn new(
        model: pick_ai::Model,
        api_key: Option<String>,
        headers: Option<std::collections::HashMap<String, String>>,
    ) -> Self {
        Self {
            model,
            api_key,
            headers,
            max_tokens: Some(4096),
        }
    }
}

#[async_trait::async_trait]
impl Summarizer for LlmSummarizer {
    async fn generate_summary(
        &self,
        conversation_text: &str,
        system_prompt: &str,
    ) -> Result<String, String> {
        let context = pick_ai::Context {
            system_prompt: Some(system_prompt.to_string()),
            messages: vec![pick_ai::Message::User(pick_ai::UserMessage::text(
                conversation_text,
            ))],
            tools: None,
        };

        let result = pick_ai::complete_simple(
            &self.model,
            context,
            self.api_key.clone(),
            self.headers.clone(),
            self.max_tokens,
            None, // temperature
            None, // reasoning
        )
        .await;

        if let Some(err) = &result.error_message {
            return Err(err.clone());
        }

        let text: String = result
            .content
            .iter()
            .filter_map(|block| match block {
                pick_ai::ContentBlock::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect();

        if text.is_empty() {
            Err("Empty summary generated".to_string())
        } else {
            Ok(text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_message_entry(role: &str, text: &str) -> SessionEntry {
        SessionEntry {
            id: uuid::Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)).to_string(),
            parent_id: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
            kind: SessionEntryKind::Message(crate::session::entries::MessageEntry {
                role: role.to_string(),
                content: serde_json::json!([{"type": "text", "text": text}]),
                api: None,
                provider: None,
                model: None,
                usage: None,
                stop_reason: None,
            }),
        }
    }

    #[test]
    fn test_estimate_tokens_user() {
        let entry = make_message_entry("user", "hello world");
        let tokens = estimate_entry_tokens(&entry);
        assert_eq!(tokens, 3); // 11 chars / 4 = 2.75 -> 3
    }

    #[test]
    fn test_estimate_tokens_assistant() {
        let entry = make_message_entry("assistant", "Hello! How can I help you today?");
        let tokens = estimate_entry_tokens(&entry);
        assert_eq!(tokens, 8); // 31 chars / 4 = 7.75 -> 8
    }

    #[test]
    fn test_find_valid_cut_points() {
        let entries = vec![
            make_message_entry("user", "hello"),
            make_message_entry("assistant", "hi"),
            make_message_entry("toolResult", "result"),
            make_message_entry("user", "next"),
        ];
        let points = find_valid_cut_points(&entries, 0, entries.len());
        assert_eq!(points, vec![0, 1, 3]); // toolResult should be excluded
    }

    #[test]
    fn test_calculate_context_tokens() {
        let usage = Usage {
            input: 100,
            output: 50,
            cache_read: 10,
            cache_write: 5,
            total_tokens: 0,
            cost: pick_ai::types::CostBreakdown::zero(),
        };
        assert_eq!(calculate_context_tokens(&usage), 165);
    }

    #[test]
    fn test_should_compact() {
        let settings = CompactionSettings {
            enabled: true,
            reserve_tokens: 1000,
            keep_recent_tokens: 20000,
        };
        assert!(should_compact(3500, 4096, &settings));
        assert!(!should_compact(3000, 4096, &settings));
        let disabled = CompactionSettings {
            enabled: false,
            ..Default::default()
        };
        assert!(!should_compact(100000, 200000, &disabled));
    }

    #[test]
    fn test_extract_file_ops() {
        let mut file_ops = FileOperations::new();
        let content = serde_json::json!([
            {"type": "toolCall", "name": "Read", "arguments": {"file_path": "/tmp/test.txt"}},
            {"type": "toolCall", "name": "Edit", "arguments": {"file_path": "/tmp/test.txt"}},
            {"type": "toolCall", "name": "Write", "arguments": {"file_path": "/tmp/new.txt"}},
        ]);
        extract_file_ops_from_content(&content, &mut file_ops);
        assert_eq!(file_ops.read, vec!["/tmp/test.txt"]);
        assert_eq!(file_ops.edited, vec!["/tmp/test.txt"]);
        assert_eq!(file_ops.written, vec!["/tmp/new.txt"]);
    }
}
