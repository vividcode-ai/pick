//! Types for TUI mode

use std::path::Path;

use crate::core::agent_mode::AgentMode;
use crate::core::resource_loader::ResourceLoader;
use crate::core::system_prompt::build_system_prompt_with_defaults_and_mode;
use async_trait::async_trait;
use pick_agent::core::state::{AgentTool, QuestionPrompt};
use pick_agent::skills::Skill;
use pick_ai::types::ContentBlock;

/// Commands sent from the agent event callback to the TUI
pub(crate) enum TuiCommand {
    StreamContent(String),
    AppendContent(String),
    ToolExecutionStart {
        tool_call_id: String,
        tool_name: String,
        args: serde_json::Value,
    },
    ToolExecutionUpdate {
        tool_call_id: String,
        partial_output: String,
    },
    ToolExecutionEnd {
        tool_call_id: String,
        tool_name: String,
        output: String,
        is_error: bool,
    },
    SetSessionTitle(String),
    SetStatus(String),
    ClearStatus,
    EndTurn,
    UpdateTodos(Vec<serde_json::Value>),
    ShowQuestions {
        prompts: Vec<QuestionPrompt>,
        response_tx: tokio::sync::oneshot::Sender<Result<Vec<Vec<String>>, String>>,
    },
    RequestApproval {
        tool_name: String,
        tool_args: String,
        permission: String,
        response_tx: tokio::sync::oneshot::Sender<Result<Vec<Vec<String>>, String>>,
    },
    GoalUpdated(serde_json::Value),
    /// Queue state update for UI feedback
    QueueUpdate {
        steer_len: usize,
        follow_up_len: usize,
        next_turn_len: usize,
    },
    /// Agent run completed — carries results for post-processing
    AgentFinished {
        result: Result<pick_agent::core::agent_loop::AgentRunResult, String>,
        prev_len: usize,
        cancel_requested: bool,
    },
    /// A queued steering message has been consumed by the agent and should
    /// be moved from "pending" to a rendered user message bubble.
    SteeringMessageConsumed(String),
    /// A queued follow-up message has been consumed by the agent (after it
    /// naturally stopped) and should be moved from "follow-up pending" to
    /// a rendered user message bubble.
    FollowUpMessageConsumed(String),
    /// Result of a /share operation — carries the gist URL or error message.
    ShareResult {
        url: Option<String>,
        error: Option<String>,
    },
}

/// TUI approval hook that shows a permission dialog in the TUI viewport
pub(crate) struct TuiApprovalHook {
    pub(crate) cmd_tx: tokio::sync::mpsc::UnboundedSender<TuiCommand>,
}

#[async_trait]
impl pick_agent::permission::hooks::PermissionRequestHook for TuiApprovalHook {
    async fn on_permission_request(
        &self,
        ctx: &pick_agent::permission::hooks::PermissionRequestContext,
    ) -> pick_agent::permission::hooks::HookAction {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _ = self.cmd_tx.send(TuiCommand::RequestApproval {
            tool_name: ctx.tool_name.clone(),
            tool_args: ctx.tool_args.clone(),
            permission: ctx.permission.clone(),
            response_tx: tx,
        });
        match rx.await {
            Ok(Ok(answers)) => {
                if answers
                    .first()
                    .and_then(|a| a.first())
                    .map(|s| s == "Allow")
                    .unwrap_or(false)
                {
                    pick_agent::permission::hooks::HookAction::Allow
                } else {
                    pick_agent::permission::hooks::HookAction::Deny {
                        reason: "Rejected by user".to_string(),
                    }
                }
            }
            _ => pick_agent::permission::hooks::HookAction::Deny {
                reason: "Approval prompt cancelled".to_string(),
            },
        }
    }
}

impl pick_agent::permission::hooks::PermissionHook for TuiApprovalHook {
    fn name(&self) -> &str {
        "tui-approval"
    }
}

/// Build context file display names (just the file name + path snippet)
pub(crate) fn build_context_display_names(resource_loader: &ResourceLoader) -> Vec<String> {
    let files = resource_loader.agents_files();
    let mut names: Vec<String> = Vec::new();
    for f in files {
        // Just show the filename (e.g., "CLAUDE.md") for compact display
        let path = std::path::Path::new(&f.path);
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&f.path)
            .to_string();
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// Build skill display names
pub(crate) fn build_skill_display_names(resource_loader: &ResourceLoader) -> Vec<String> {
    resource_loader
        .skills()
        .iter()
        .map(|s| s.name.clone())
        .collect()
}

/// Build the system prompt including tools, skills, context files, custom prompt,
/// APPEND_SYSTEM.md content, CLI flags, and provider/model info.
pub(crate) fn build_prompt(
    tools: &[AgentTool],
    resource_loader: &ResourceLoader,
    cwd: &Path,
    provider: &str,
    model_id: &str,
    override_prompt: Option<&str>,
    extra_append: &[String],
    agent_mode: Option<&AgentMode>,
    enable_skills: bool,
) -> String {
    let custom_prompt = override_prompt.or_else(|| resource_loader.system_prompt());
    let loader_append = resource_loader.append_system_prompt().join("\n");
    let mut append_parts: Vec<String> = Vec::new();
    append_parts.extend(extra_append.iter().cloned());
    if !loader_append.is_empty() {
        append_parts.push(loader_append);
    }
    let append_text = if append_parts.is_empty() {
        format!("Provider: {}  Model: {}", provider, model_id)
    } else {
        format!(
            "{}\nProvider: {}  Model: {}",
            append_parts.join("\n"),
            provider,
            model_id
        )
    };
    // When skills are disabled, pass an empty slice so the system prompt
    // excludes <available_skills>...</available_skills>
    let skills: &[Skill] = if enable_skills {
        resource_loader.skills()
    } else {
        &[]
    };
    build_system_prompt_with_defaults_and_mode(
        tools,
        skills,
        resource_loader.agents_files(),
        custom_prompt,
        Some(&append_text),
        cwd,
        agent_mode,
    )
}

/// Check if a session title is a default placeholder (not user-customized)
pub(crate) fn is_default_session_title(name: &str) -> bool {
    name.starts_with("New session - ") || name.starts_with("Child session - ")
}

/// Combine content blocks into a single display string with ANSI markers.
/// Thinking blocks are wrapped in italic+gray ANSI sequences (same format as live streaming).
pub(crate) fn combine_content_blocks(
    content: &[ContentBlock],
    hide_thinking: bool,
    show_images: bool,
    block_images: bool,
) -> String {
    let mut combined = String::new();
    for block in content {
        match block {
            ContentBlock::Text(t) => {
                combined.push_str(&t.text);
            }
            ContentBlock::Thinking(t)
                if !t.thinking.is_empty() && !t.redacted && !hide_thinking =>
            {
                if !combined.is_empty() && !combined.ends_with('\n') {
                    combined.push('\n');
                }
                combined.push_str(&format!(
                    "\x1b[3m\x1b[38;2;128;128;128m{}\x1b[23m\x1b[39m\n\n",
                    t.thinking.trim_end()
                ));
            }
            ContentBlock::Image(_) if block_images => {
                if !combined.is_empty() && !combined.ends_with('\n') {
                    combined.push('\n');
                }
                combined.push_str("\x1b[38;2;128;128;128m[Image blocked by settings]\x1b[39m\n");
            }
            ContentBlock::Image(_) if !show_images => {
                if !combined.is_empty() && !combined.ends_with('\n') {
                    combined.push('\n');
                }
                combined.push_str("\x1b[38;2;128;128;128m[Image]\x1b[39m\n");
            }
            ContentBlock::Image(_) => {
                if !combined.is_empty() && !combined.ends_with('\n') {
                    combined.push('\n');
                }
                combined.push_str("[Image]\n");
            }
            _ => {}
        }
    }
    combined
}
