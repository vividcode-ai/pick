use serde_json::Value;

use super::compaction::estimate_tokens;
use super::utils::{
    FileOperations, compute_file_lists, create_file_ops, extract_file_ops_from_message,
    format_file_operations,
};
use crate::core::messages;

/// File-operation details stored on generated branch summary entries
#[derive(Debug, Clone)]
pub struct BranchSummaryDetails {
    pub read_files: Vec<String>,
    pub modified_files: Vec<String>,
}

/// Prepared branch content for summarization
#[derive(Debug, Clone)]
pub struct BranchPreparation {
    pub messages: Vec<Value>,
    pub file_ops: FileOperations,
    pub total_tokens: usize,
}

/// Entries selected for branch summarization
#[derive(Debug, Clone)]
pub struct CollectEntriesResult {
    pub entries: Vec<Value>,
    pub common_ancestor_id: Option<String>,
}

/// Options for generating a branch summary
#[derive(Debug, Clone)]
pub struct GenerateBranchSummaryOptions {
    pub custom_instructions: Option<String>,
    pub replace_instructions: bool,
    pub reserve_tokens: usize,
}

impl Default for GenerateBranchSummaryOptions {
    fn default() -> Self {
        Self {
            custom_instructions: None,
            replace_instructions: false,
            reserve_tokens: 16384,
        }
    }
}

fn get_message_from_entry(entry: &Value) -> Option<Value> {
    match entry.get("type").and_then(|v| v.as_str()) {
        Some("message") => {
            let role = entry
                .get("message")
                .and_then(|m| m.get("role"))
                .and_then(|r| r.as_str())
                .unwrap_or("");
            if role == "toolResult" {
                return None;
            }
            entry.get("message").cloned()
        }
        Some("custom_message") => {
            let custom_type = entry
                .get("customType")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let content = entry.get("content").cloned().unwrap_or(Value::Null);
            let display = entry
                .get("display")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let details = entry.get("details").cloned();
            let ts = entry.get("timestamp").and_then(|v| v.as_i64()).unwrap_or(0);
            Some(messages::create_custom_message(
                custom_type.to_string(),
                content,
                display,
                details,
                ts,
            ))
        }
        Some("branch_summary") => {
            let summary = entry.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            let from_id = entry.get("fromId").and_then(|v| v.as_str()).unwrap_or("");
            let ts = entry.get("timestamp").and_then(|v| v.as_i64()).unwrap_or(0);
            Some(messages::create_branch_summary_message(
                summary.to_string(),
                from_id.to_string(),
                ts,
            ))
        }
        Some("compaction") => {
            let summary = entry.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            let tokens_before = entry
                .get("tokensBefore")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let ts = entry.get("timestamp").and_then(|v| v.as_i64()).unwrap_or(0);
            Some(messages::create_compaction_summary_message(
                summary.to_string(),
                tokens_before,
                ts,
            ))
        }
        _ => None,
    }
}

/// Prepare branch entries for summarization within an optional token budget
pub fn prepare_branch_entries(entries: &[Value], token_budget: usize) -> BranchPreparation {
    let mut messages: Vec<Value> = Vec::new();
    let mut file_ops = create_file_ops();
    let mut total_tokens = 0usize;

    for entry in entries {
        if entry.get("type").and_then(|v| v.as_str()) == Some("branch_summary") {
            if entry.get("fromHook") != Some(&Value::Bool(true)) {
                if let Some(details) = entry.get("details") {
                    if let Some(read_files) = details.get("readFiles").and_then(|v| v.as_array()) {
                        for f in read_files {
                            if let Some(s) = f.as_str() {
                                file_ops.read.insert(s.to_string());
                            }
                        }
                    }
                    if let Some(modified_files) =
                        details.get("modifiedFiles").and_then(|v| v.as_array())
                    {
                        for f in modified_files {
                            if let Some(s) = f.as_str() {
                                file_ops.edited.insert(s.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    for entry in entries.iter().rev() {
        let message = get_message_from_entry(entry);
        if let Some(msg) = message {
            extract_file_ops_from_message(&msg, &mut file_ops);
            let tokens = estimate_tokens(&msg);
            if token_budget > 0 && total_tokens + tokens > token_budget {
                if let Some(entry_type) = entry.get("type").and_then(|v| v.as_str()) {
                    if entry_type == "compaction" || entry_type == "branch_summary" {
                        if total_tokens < (token_budget as f64 * 0.9) as usize {
                            messages.insert(0, msg);
                            total_tokens += tokens;
                        }
                    }
                }
                break;
            }
            messages.insert(0, msg);
            total_tokens += tokens;
        }
    }

    BranchPreparation {
        messages,
        file_ops,
        total_tokens,
    }
}

/// Generate a summary for abandoned branch entries
pub async fn generate_branch_summary(
    entries: &[Value],
    options: &GenerateBranchSummaryOptions,
) -> Result<BranchSummaryResult, String> {
    let reserve_tokens = options.reserve_tokens;
    // Use a large default context window
    let context_window = 128000;
    let token_budget = context_window - reserve_tokens;

    let BranchPreparation {
        messages, file_ops, ..
    } = prepare_branch_entries(entries, token_budget);

    if messages.is_empty() {
        return Ok(BranchSummaryResult {
            summary: "No content to summarize".to_string(),
            read_files: Vec::new(),
            modified_files: Vec::new(),
        });
    }

    let (read_files, modified_files) = compute_file_lists(&file_ops);
    let summary = format!(
        "{}{}{}",
        BRANCH_SUMMARY_PREAMBLE,
        format!(
            "Summary of branch with {} messages and {} file operations.",
            messages.len(),
            read_files.len() + modified_files.len()
        ),
        format_file_operations(&read_files, &modified_files),
    );

    Ok(BranchSummaryResult {
        summary,
        read_files,
        modified_files,
    })
}

#[derive(Debug, Clone)]
pub struct BranchSummaryResult {
    pub summary: String,
    pub read_files: Vec<String>,
    pub modified_files: Vec<String>,
}

pub const BRANCH_SUMMARY_PREAMBLE: &str = "The user explored a different conversation branch before returning here.\nSummary of that exploration:\n\n";

pub const BRANCH_SUMMARY_PROMPT: &str =
    "Create a structured summary of this conversation branch for context when returning later.
Use this EXACT format:

## Goal
[What was the user trying to accomplish in this branch?]

## Constraints & Preferences
- [Any constraints, preferences, or requirements mentioned]
- [Or \"(none)\" if none were mentioned]

## Progress
### Done
- [x] [Completed tasks/changes]

### In Progress
- [ ] [Work that was started but not finished]

### Blocked
- [Issues preventing progress, if any]

## Key Decisions
- **[Decision]**: [Brief rationale]

## Next Steps
1. [What should happen next to continue this work]

Keep each section concise. Preserve exact file paths, function names, and error messages.";
