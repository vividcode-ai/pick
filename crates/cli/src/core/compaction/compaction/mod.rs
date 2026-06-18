pub mod types;

use serde_json::Value;

use super::utils::{
    FileOperations, compute_file_lists, create_file_ops, extract_file_ops_from_message,
    format_file_operations,
};
use crate::core::messages;
pub use types::*;

struct UsageInfo {
    usage: Value,
    index: usize,
}

pub const SUMMARIZATION_SYSTEM_PROMPT: &str =
    "Summarize the conversation so far, preserving key decisions, file paths, and rationale.";

pub fn estimate_tokens(message: &Value) -> usize {
    let role = message.get("role").and_then(|v| v.as_str()).unwrap_or("");
    let chars = match role {
        "user" => {
            let content = message.get("content");
            match content {
                Some(Value::String(s)) => s.len(),
                Some(Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|block| block.get("text").and_then(|v| v.as_str()))
                    .map(|s| s.len())
                    .sum(),
                _ => 0,
            }
        }
        "assistant" => {
            let mut chars = 0usize;
            if let Some(Value::Array(content)) = message.get("content") {
                for block in content {
                    match block.get("type").and_then(|v| v.as_str()) {
                        Some("text") => {
                            chars += block
                                .get("text")
                                .and_then(|v| v.as_str())
                                .map(|s| s.len())
                                .unwrap_or(0);
                        }
                        Some("thinking") => {
                            chars += block
                                .get("thinking")
                                .and_then(|v| v.as_str())
                                .map(|s| s.len())
                                .unwrap_or(0);
                        }
                        Some("toolCall") => {
                            let name_len = block
                                .get("name")
                                .and_then(|v| v.as_str())
                                .map(|s| s.len())
                                .unwrap_or(0);
                            let args_str =
                                serde_json::to_string(&block.get("arguments")).unwrap_or_default();
                            chars += name_len + args_str.len();
                        }
                        _ => {}
                    }
                }
            }
            chars
        }
        "custom" | "toolResult" => match message.get("content") {
            Some(Value::String(s)) => s.len(),
            Some(Value::Array(arr)) => arr
                .iter()
                .map(|block| {
                    let mut c = block
                        .get("text")
                        .and_then(|v| v.as_str())
                        .map(|s| s.len())
                        .unwrap_or(0);
                    if block.get("type").and_then(|v| v.as_str()) == Some("image") {
                        c += 4800;
                    }
                    c
                })
                .sum(),
            _ => 0,
        },
        "bashExecution" => {
            let cmd_len = message
                .get("command")
                .and_then(|v| v.as_str())
                .map(|s| s.len())
                .unwrap_or(0);
            let out_len = message
                .get("output")
                .and_then(|v| v.as_str())
                .map(|s| s.len())
                .unwrap_or(0);
            cmd_len + out_len
        }
        "branchSummary" | "compactionSummary" => message
            .get("summary")
            .and_then(|v| v.as_str())
            .map(|s| s.len())
            .unwrap_or(0),
        _ => 0,
    };
    (chars + 3) / 4
}

pub fn estimate_context_tokens(messages: &[Value]) -> ContextUsageEstimate {
    let usage_info = get_last_assistant_usage_info(messages);

    match usage_info {
        None => {
            let estimated: usize = messages.iter().map(|m| estimate_tokens(m)).sum();
            ContextUsageEstimate {
                tokens: estimated,
                usage_tokens: 0,
                trailing_tokens: estimated,
                last_usage_index: None,
            }
        }
        Some(info) => {
            let usage_tokens = calculate_context_tokens(&info.usage);
            let trailing_tokens: usize = messages[(info.index + 1)..]
                .iter()
                .map(|m| estimate_tokens(m))
                .sum();
            ContextUsageEstimate {
                tokens: usage_tokens + trailing_tokens,
                usage_tokens,
                trailing_tokens,
                last_usage_index: Some(info.index),
            }
        }
    }
}

pub fn should_compact(
    context_tokens: usize,
    context_window: usize,
    settings: &CompactionSettings,
) -> bool {
    if !settings.enabled {
        return false;
    }
    context_tokens > context_window - settings.reserve_tokens
}

pub fn find_cut_point(
    entries: &[Value],
    start_index: usize,
    end_index: usize,
    keep_recent_tokens: usize,
) -> CutPointResult {
    let cut_points = find_valid_cut_points(entries, start_index, end_index);

    if cut_points.is_empty() {
        return CutPointResult {
            first_kept_entry_index: start_index,
            turn_start_index: None,
            is_split_turn: false,
        };
    }

    let mut accumulated_tokens = 0usize;
    let mut cut_index = cut_points[0];

    for i in (start_index..end_index).rev() {
        let entry = &entries[i];
        if entry.get("type").and_then(|v| v.as_str()) != Some("message") {
            continue;
        }
        let msg = entry.get("message").cloned().unwrap_or(Value::Null);
        let message_tokens = estimate_tokens(&msg);
        accumulated_tokens += message_tokens;
        if accumulated_tokens >= keep_recent_tokens {
            for &c in &cut_points {
                if c >= i {
                    cut_index = c;
                    break;
                }
            }
            break;
        }
    }

    while cut_index > start_index {
        let prev_entry = &entries[cut_index - 1];
        match prev_entry.get("type").and_then(|v| v.as_str()) {
            Some("compaction") => break,
            Some("message") => break,
            _ => {
                cut_index -= 1;
            }
        }
    }

    let cut_entry = &entries[cut_index];
    let is_user_message = cut_entry.get("type").and_then(|v| v.as_str()) == Some("message")
        && cut_entry
            .get("message")
            .and_then(|m| m.get("role"))
            .and_then(|r| r.as_str())
            == Some("user");
    let turn_start_index = if is_user_message {
        None
    } else {
        find_turn_start_index(entries, cut_index, start_index)
    };

    CutPointResult {
        first_kept_entry_index: cut_index,
        turn_start_index,
        is_split_turn: !is_user_message && turn_start_index.is_some(),
    }
}

fn find_valid_cut_points(entries: &[Value], start_index: usize, end_index: usize) -> Vec<usize> {
    let mut cut_points = Vec::new();
    for i in start_index..end_index {
        let entry = &entries[i];
        match entry.get("type").and_then(|v| v.as_str()) {
            Some("message") => {
                let role = entry
                    .get("message")
                    .and_then(|m| m.get("role"))
                    .and_then(|r| r.as_str())
                    .unwrap_or("");
                match role {
                    "bashExecution" | "custom" | "branchSummary" | "compactionSummary" | "user"
                    | "assistant" => {
                        cut_points.push(i);
                    }
                    "toolResult" => {}
                    _ => {}
                }
            }
            Some("branch_summary") | Some("custom_message") => {
                cut_points.push(i);
            }
            _ => {}
        }
    }
    cut_points
}

fn find_turn_start_index(
    entries: &[Value],
    entry_index: usize,
    start_index: usize,
) -> Option<usize> {
    for i in (start_index..=entry_index).rev() {
        let entry = &entries[i];
        match entry.get("type").and_then(|v| v.as_str()) {
            Some("branch_summary") | Some("custom_message") => return Some(i),
            Some("message") => {
                let role = entry
                    .get("message")
                    .and_then(|m| m.get("role"))
                    .and_then(|r| r.as_str())
                    .unwrap_or("");
                if role == "user" || role == "bashExecution" {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_file_operations(
    messages: &[Value],
    entries: &[Value],
    prev_compaction_index: isize,
) -> FileOperations {
    let mut file_ops = create_file_ops();
    if prev_compaction_index >= 0 {
        let prev_compaction = &entries[prev_compaction_index as usize];
        if prev_compaction.get("fromHook") != Some(&Value::Bool(true)) {
            if let Some(details) = prev_compaction.get("details") {
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
    for msg in messages {
        extract_file_ops_from_message(msg, &mut file_ops);
    }
    file_ops
}

pub fn prepare_compaction(
    path_entries: &[Value],
    settings: &CompactionSettings,
) -> Option<CompactionPreparation> {
    if path_entries.is_empty() {
        return None;
    }
    let last_entry = path_entries.last()?;
    if last_entry.get("type").and_then(|v| v.as_str()) == Some("compaction") {
        return None;
    }

    let mut prev_compaction_index = -1isize;
    for i in (0..path_entries.len()).rev() {
        if path_entries[i].get("type").and_then(|v| v.as_str()) == Some("compaction") {
            prev_compaction_index = i as isize;
            break;
        }
    }

    let mut previous_summary: Option<String> = None;
    let boundary_start: usize;
    if prev_compaction_index >= 0 {
        let prev_compaction = &path_entries[prev_compaction_index as usize];
        previous_summary = prev_compaction
            .get("summary")
            .and_then(|v| v.as_str())
            .map(String::from);
        let first_kept_entry_id = prev_compaction
            .get("firstKeptEntryId")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let first_kept_index = path_entries
            .iter()
            .position(|e| e.get("id").and_then(|v| v.as_str()) == Some(first_kept_entry_id));
        boundary_start = first_kept_index.unwrap_or(prev_compaction_index as usize + 1);
    } else {
        boundary_start = 0;
    }
    let boundary_end = path_entries.len();

    let tokens_before = 0;

    let cut_point = find_cut_point(
        path_entries,
        boundary_start,
        boundary_end,
        settings.keep_recent_tokens,
    );
    let first_kept_entry = &path_entries[cut_point.first_kept_entry_index];
    let first_kept_entry_id = first_kept_entry
        .get("id")
        .and_then(|v| v.as_str())
        .map(String::from)?;

    let history_end = if cut_point.is_split_turn {
        cut_point
            .turn_start_index
            .unwrap_or(cut_point.first_kept_entry_index)
    } else {
        cut_point.first_kept_entry_index
    };

    let messages_to_summarize: Vec<Value> = path_entries[boundary_start..history_end]
        .iter()
        .filter_map(|e| get_message_from_entry_for_compaction(e))
        .collect();

    let turn_prefix_messages: Vec<Value> = if cut_point.is_split_turn {
        if let Some(turn_start) = cut_point.turn_start_index {
            path_entries[turn_start..cut_point.first_kept_entry_index]
                .iter()
                .filter_map(|e| get_message_from_entry_for_compaction(e))
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let mut file_ops =
        extract_file_operations(&messages_to_summarize, path_entries, prev_compaction_index);
    for msg in &turn_prefix_messages {
        extract_file_ops_from_message(msg, &mut file_ops);
    }

    Some(CompactionPreparation {
        first_kept_entry_id,
        messages_to_summarize,
        turn_prefix_messages,
        is_split_turn: cut_point.is_split_turn,
        tokens_before,
        previous_summary,
        file_ops,
        settings: settings.clone(),
    })
}

fn get_message_from_entry(entry: &Value) -> Option<Value> {
    match entry.get("type").and_then(|v| v.as_str()) {
        Some("message") => entry.get("message").cloned(),
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
            let timestamp = entry.get("timestamp").and_then(|v| v.as_i64()).unwrap_or(0);
            Some(messages::create_custom_message(
                custom_type.to_string(),
                content,
                display,
                details,
                timestamp,
            ))
        }
        Some("branch_summary") => {
            let summary = entry.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            let from_id = entry.get("fromId").and_then(|v| v.as_str()).unwrap_or("");
            let timestamp = entry.get("timestamp").and_then(|v| v.as_i64()).unwrap_or(0);
            Some(messages::create_branch_summary_message(
                summary.to_string(),
                from_id.to_string(),
                timestamp,
            ))
        }
        Some("compaction") => {
            let summary = entry.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            let tokens_before = entry
                .get("tokensBefore")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let timestamp = entry.get("timestamp").and_then(|v| v.as_i64()).unwrap_or(0);
            Some(messages::create_compaction_summary_message(
                summary.to_string(),
                tokens_before,
                timestamp,
            ))
        }
        _ => None,
    }
}

fn get_message_from_entry_for_compaction(entry: &Value) -> Option<Value> {
    if entry.get("type").and_then(|v| v.as_str()) == Some("compaction") {
        return None;
    }
    get_message_from_entry(entry)
}

pub fn calculate_context_tokens(usage: &Value) -> usize {
    usage
        .get("totalTokens")
        .and_then(|v| v.as_u64())
        .or_else(|| {
            let input = usage.get("input").and_then(|v| v.as_u64()).unwrap_or(0);
            let output = usage.get("output").and_then(|v| v.as_u64()).unwrap_or(0);
            let cache_read = usage.get("cacheRead").and_then(|v| v.as_u64()).unwrap_or(0);
            let cache_write = usage
                .get("cacheWrite")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            Some(input + output + cache_read + cache_write)
        })
        .unwrap_or(0) as usize
}

fn get_last_assistant_usage_info(messages: &[Value]) -> Option<UsageInfo> {
    for (i, msg) in messages.iter().enumerate().rev() {
        if msg.get("role").and_then(|v| v.as_str()) == Some("assistant") {
            let stop_reason = msg.get("stopReason").and_then(|v| v.as_str()).unwrap_or("");
            if stop_reason != "aborted" && stop_reason != "error" {
                if let Some(usage) = msg.get("usage") {
                    return Some(UsageInfo {
                        usage: usage.clone(),
                        index: i,
                    });
                }
            }
        }
    }
    None
}

pub fn get_last_assistant_usage(entries: &[Value]) -> Option<Value> {
    for entry in entries.iter().rev() {
        let msg = entry.get("message")?;
        let stop_reason = msg.get("stopReason").and_then(|v| v.as_str()).unwrap_or("");
        if stop_reason != "aborted" && stop_reason != "error" {
            if let Some(usage) = msg.get("usage") {
                return Some(usage.clone());
            }
        }
    }
    None
}

const SUMMARIZATION_PROMPT: &str = "The messages above are a conversation to summarize. Create a structured context checkpoint summary that another LLM will use to continue the work.

Use this EXACT format:

## Goal
[What is the user trying to accomplish? Can be multiple items if the session covers different tasks.]

## Constraints & Preferences
- [Any constraints, preferences, or requirements mentioned by user]
- [Or \"(none)\" if none were mentioned]

## Progress
### Done
- [x] [Completed tasks/changes]

### In Progress
- [ ] [Current work]

### Blocked
- [Issues preventing progress, if any]

## Key Decisions
- **[Decision]**: [Brief rationale]

## Next Steps
1. [Ordered list of what should happen next]

## Critical Context
- [Any data, examples, or references needed to continue]
- [Or \"(none)\" if not applicable]

Keep each section concise. Preserve exact file paths, function names, and error messages.";

const UPDATE_SUMMARIZATION_PROMPT: &str = "The messages above are NEW conversation messages to incorporate into the existing summary provided in <previous-summary> tags.

Update the existing structured summary with new information. RULES:
- PRESERVE all existing information from the previous summary
- ADD new progress, decisions, and context from the new messages
- UPDATE the Progress section: move items from \"In Progress\" to \"Done\" when completed
- UPDATE \"Next Steps\" based on what was accomplished
- PRESERVE exact file paths, function names, and error messages
- If something is no longer relevant, you may remove it

Use this EXACT format:

## Goal
[Preserve existing goals, add new ones if the task expanded]

## Constraints & Preferences
- [Preserve existing, add new ones discovered]

## Progress
### Done
- [x] [Include previously done items AND newly completed items]

### In Progress
- [ ] [Current work - update based on progress]

### Blocked
- [Current blockers - remove if resolved]

## Key Decisions
- **[Decision]**: [Brief rationale] (preserve all previous, add new)

## Next Steps
1. [Update based on current state]

## Critical Context
- [Preserve important context, add new if needed]

Keep each section concise. Preserve exact file paths, function names, and error messages.";

const TURN_PREFIX_SUMMARIZATION_PROMPT: &str =
    "This is the PREFIX of a turn that was too large to keep. The SUFFIX (recent work) is retained.

Summarize the prefix to provide context for the retained suffix:

## Original Request
[What did the user ask for in this turn?]

## Early Progress
- [Key decisions and work done in the prefix]

## Context for Suffix
- [Information needed to understand the retained recent work]

Be concise. Focus on what's needed to understand the kept suffix.";

pub async fn generate_summary(
    current_messages: &[Value],
    model: &pick_ai::Model,
    reserve_tokens: usize,
    api_key: &str,
    headers: Option<std::collections::HashMap<String, String>>,
    custom_instructions: Option<&str>,
    previous_summary: Option<&str>,
    thinking_level: Option<&str>,
) -> Result<String, CompactionError> {
    let max_tokens = Some(std::cmp::min(
        (reserve_tokens as f64 * 0.8) as u64,
        if model.max_tokens > 0 {
            model.max_tokens
        } else {
            u64::MAX
        },
    ));

    let base_prompt = if previous_summary.is_some() {
        UPDATE_SUMMARIZATION_PROMPT
    } else {
        SUMMARIZATION_PROMPT
    };

    let prompt = if let Some(custom) = custom_instructions {
        format!("{}\n\nAdditional focus: {}", base_prompt, custom)
    } else {
        base_prompt.to_string()
    };

    let conversation_text = super::utils::serialize_conversation(current_messages);
    let mut prompt_text = format!("<conversation>\n{}</conversation>\n\n", conversation_text);
    if let Some(prev) = previous_summary {
        prompt_text.push_str(&format!(
            "<previous-summary>\n{}</previous-summary>\n\n",
            prev
        ));
    }
    prompt_text.push_str(&prompt);

    let summarization_messages = vec![pick_ai::Message::User(pick_ai::UserMessage::new(vec![
        pick_ai::ContentBlock::text(prompt_text),
    ]))];

    let context = pick_ai::Context {
        system_prompt: Some(SUMMARIZATION_SYSTEM_PROMPT.to_string()),
        messages: summarization_messages,
        tools: None,
    };

    let reasoning = if model.reasoning && thinking_level.is_some_and(|l| l != "off") {
        thinking_level.map(String::from)
    } else {
        None
    };

    let result = pick_ai::complete_simple(
        model,
        context,
        Some(api_key.to_string()),
        headers,
        max_tokens,
        None,
        reasoning,
    )
    .await;

    match result.stop_reason {
        pick_ai::StopReason::Aborted => Err(CompactionError {
            code: "aborted".to_string(),
            message: result
                .error_message
                .unwrap_or_else(|| "Summarization aborted".to_string()),
        }),
        pick_ai::StopReason::Error => Err(CompactionError {
            code: "summarization_failed".to_string(),
            message: format!(
                "Summarization failed: {}",
                result
                    .error_message
                    .unwrap_or_else(|| "Unknown error".to_string())
            ),
        }),
        _ => {
            let text: String = result
                .content
                .iter()
                .filter_map(|c| match c {
                    pick_ai::ContentBlock::Text(t) => Some(t.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            Ok(text)
        }
    }
}

async fn generate_turn_prefix_summary(
    messages: &[Value],
    model: &pick_ai::Model,
    reserve_tokens: usize,
    api_key: &str,
    headers: Option<std::collections::HashMap<String, String>>,
    thinking_level: Option<&str>,
) -> Result<String, CompactionError> {
    let max_tokens = Some(std::cmp::min(
        (reserve_tokens as f64 * 0.5) as u64,
        if model.max_tokens > 0 {
            model.max_tokens
        } else {
            u64::MAX
        },
    ));

    let conversation_text = super::utils::serialize_conversation(messages);
    let prompt_text = format!(
        "<conversation>\n{}</conversation>\n\n{}",
        conversation_text, TURN_PREFIX_SUMMARIZATION_PROMPT
    );

    let summarization_messages = vec![pick_ai::Message::User(pick_ai::UserMessage::new(vec![
        pick_ai::ContentBlock::text(prompt_text),
    ]))];

    let context = pick_ai::Context {
        system_prompt: Some(SUMMARIZATION_SYSTEM_PROMPT.to_string()),
        messages: summarization_messages,
        tools: None,
    };

    let reasoning = if model.reasoning && thinking_level.is_some_and(|l| l != "off") {
        thinking_level.map(String::from)
    } else {
        None
    };

    let result = pick_ai::complete_simple(
        model,
        context,
        Some(api_key.to_string()),
        headers,
        max_tokens,
        None,
        reasoning,
    )
    .await;

    match result.stop_reason {
        pick_ai::StopReason::Aborted => Err(CompactionError {
            code: "aborted".to_string(),
            message: result
                .error_message
                .unwrap_or_else(|| "Turn prefix summarization aborted".to_string()),
        }),
        pick_ai::StopReason::Error => Err(CompactionError {
            code: "summarization_failed".to_string(),
            message: format!(
                "Turn prefix summarization failed: {}",
                result
                    .error_message
                    .unwrap_or_else(|| "Unknown error".to_string())
            ),
        }),
        _ => {
            let text: String = result
                .content
                .iter()
                .filter_map(|c| match c {
                    pick_ai::ContentBlock::Text(t) => Some(t.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            Ok(text)
        }
    }
}

pub async fn compact(
    preparation: &CompactionPreparation,
    model: &pick_ai::Model,
    api_key: &str,
    headers: Option<std::collections::HashMap<String, String>>,
    custom_instructions: Option<&str>,
    thinking_level: Option<&str>,
) -> Result<CompactionResult<CompactionDetails>, CompactionError> {
    let summary = if preparation.is_split_turn && !preparation.turn_prefix_messages.is_empty() {
        let history_result = if !preparation.messages_to_summarize.is_empty() {
            generate_summary(
                &preparation.messages_to_summarize,
                model,
                preparation.settings.reserve_tokens,
                api_key,
                headers.clone(),
                custom_instructions,
                preparation.previous_summary.as_deref(),
                thinking_level,
            )
            .await?
        } else {
            "No prior history.".to_string()
        };

        let turn_prefix_result = generate_turn_prefix_summary(
            &preparation.turn_prefix_messages,
            model,
            preparation.settings.reserve_tokens,
            api_key,
            headers.clone(),
            thinking_level,
        )
        .await?;

        format!(
            "{}\n\n---\n\n**Turn Context (split turn):**\n\n{}",
            history_result, turn_prefix_result
        )
    } else {
        generate_summary(
            &preparation.messages_to_summarize,
            model,
            preparation.settings.reserve_tokens,
            api_key,
            headers,
            custom_instructions,
            preparation.previous_summary.as_deref(),
            thinking_level,
        )
        .await?
    };

    let (read_files, modified_files) = compute_file_lists(&preparation.file_ops);
    let mut full_summary = summary;
    full_summary.push_str(&format_file_operations(&read_files, &modified_files));

    Ok(CompactionResult {
        summary: full_summary,
        first_kept_entry_id: preparation.first_kept_entry_id.clone(),
        tokens_before: preparation.tokens_before,
        details: Some(CompactionDetails {
            read_files,
            modified_files,
        }),
    })
}
