use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::core::state::AgentTool;
use crate::skills::{Skill, format_skills_for_prompt};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone)]
pub struct ContextFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ContextFileRef<'a> {
    pub path: &'a str,
    pub content: &'a str,
}

/// Options for building a system prompt
pub struct BuildSystemPromptOptions<'a> {
    /// Custom system prompt (replaces default)
    pub custom_prompt: Option<&'a str>,
    /// Default prompt text to use when no custom_prompt is given (pass None for empty base)
    pub default_prompt: Option<&'a str>,
    /// Tools to include in prompt
    pub selected_tools: Option<&'a [String]>,
    /// One-line tool snippets keyed by tool name
    pub tool_snippets: Option<&'a HashMap<String, String>>,
    /// Additional guideline bullets
    pub prompt_guidelines: Option<&'a [String]>,
    /// Text to append to system prompt
    pub append_system_prompt: Option<&'a str>,
    /// Working directory
    pub cwd: &'a Path,
    /// Pre-loaded context files
    pub context_files: Option<&'a [ContextFileRef<'a>]>,
    /// Pre-loaded skills
    pub skills: Option<&'a [Skill]>,
    /// Agent mode ("build" / "plan") for mode-specific prompt injection
    pub agent_mode: Option<&'a str>,
}

// ============================================================================
// Constants & Path Helpers
// ============================================================================

pub const CONFIG_DIR_NAME: &str = ".pick";

pub fn get_agent_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_default();
    home.join(".pick").join("agent")
}

fn resolve_path(p: &str, cwd: &Path) -> PathBuf {
    let path = Path::new(p);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn canonicalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

// ============================================================================
// System Prompt File Discovery
// ============================================================================

/// Discover SYSTEM.md — project-level takes priority over global
pub fn discover_custom_prompt(agent_dir: &Path, cwd: &Path) -> Option<String> {
    let project_path = cwd.join(CONFIG_DIR_NAME).join("SYSTEM.md");
    if project_path.exists() {
        return std::fs::read_to_string(&project_path).ok();
    }

    let global_path = agent_dir.join("SYSTEM.md");
    if global_path.exists() {
        return std::fs::read_to_string(&global_path).ok();
    }

    None
}

/// Discover APPEND_SYSTEM.md — returns content from both project and global
pub fn discover_append_prompt(agent_dir: &Path, cwd: &Path) -> Vec<String> {
    let mut prompts = Vec::new();

    let project_path = cwd.join(CONFIG_DIR_NAME).join("APPEND_SYSTEM.md");
    if project_path.exists()
        && let Ok(content) = std::fs::read_to_string(&project_path)
    {
        prompts.push(content);
    }

    let global_path = agent_dir.join("APPEND_SYSTEM.md");
    if global_path.exists()
        && let Ok(content) = std::fs::read_to_string(&global_path)
    {
        prompts.push(content);
    }

    prompts
}

/// Resolve a prompt input: if the value is an existing file path, read its content; otherwise return as-is
#[allow(dead_code)]
fn resolve_prompt_input(input: &str, description: &str) -> Option<String> {
    let path = Path::new(input);
    if path.exists() {
        match std::fs::read_to_string(path) {
            Ok(content) => return Some(content),
            Err(e) => {
                tracing::warn!("Could not read {} file {}: {}", description, input, e);
            }
        }
    }
    Some(input.to_string())
}

// ============================================================================
// Context File Loading (AGENTS.md / CLAUDE.md)
// ============================================================================

fn load_context_file_from_dir(dir: &Path) -> Option<ContextFile> {
    let candidates = ["AGENTS.md", "AGENTS.MD", "CLAUDE.md", "CLAUDE.MD"];
    for filename in &candidates {
        let file_path = dir.join(filename);
        if file_path.exists() {
            match std::fs::read_to_string(&file_path) {
                Ok(content) => {
                    return Some(ContextFile {
                        path: file_path.to_string_lossy().to_string(),
                        content,
                    });
                }
                Err(e) => {
                    tracing::warn!("Could not read {:?}: {}", file_path, e);
                }
            }
        }
    }
    None
}

/// Load AGENTS.md / CLAUDE.md context files from global dir and ancestor directories
pub fn load_project_context_files(cwd: &Path, agent_dir: &Path) -> Vec<ContextFile> {
    let resolved_cwd = canonicalize_path(cwd);
    let resolved_agent_dir = canonicalize_path(agent_dir);

    let mut context_files: Vec<ContextFile> = Vec::new();
    let mut seen_paths = HashSet::new();

    // Global context (agent dir)
    let global_context = load_context_file_from_dir(&resolved_agent_dir);
    if let Some(ctx) = global_context
        && seen_paths.insert(canonicalize_path(Path::new(&ctx.path)))
    {
        context_files.push(ctx);
    }

    // Ancestor context files
    let mut ancestor_files: Vec<ContextFile> = Vec::new();
    let mut current_dir = Some(resolved_cwd.as_path());

    while let Some(dir) = current_dir {
        let ctx = load_context_file_from_dir(dir);
        if let Some(c) = ctx {
            let canon = canonicalize_path(Path::new(&c.path));
            if seen_paths.insert(canon) {
                ancestor_files.push(c);
            }
        }

        if let Some(parent) = dir.parent() {
            if parent == dir {
                break;
            }
            current_dir = Some(parent);
        } else {
            break;
        }
    }

    // Reverse to get top-down order (root-most last in the original)
    ancestor_files.reverse();
    context_files.extend(ancestor_files);

    context_files
}

// ============================================================================
// Core System Prompt Builder
// ============================================================================

/// Build the system prompt with tools, guidelines, and context
pub fn build_system_prompt(options: BuildSystemPromptOptions) -> String {
    let custom_prompt = options.custom_prompt;
    let selected_tools = options.selected_tools;
    let _tool_snippets = options.tool_snippets;
    let _prompt_guidelines = options.prompt_guidelines;
    let append_system_prompt = options.append_system_prompt;
    let cwd = options.cwd;
    let context_files = options.context_files.unwrap_or(&[]);
    let skills = options.skills.unwrap_or(&[]);
    let agent_mode = options.agent_mode;

    let prompt_cwd = cwd.to_string_lossy().replace('\\', "/");

    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();

    let append_section = append_system_prompt
        .filter(|s| !s.is_empty())
        .map(|s| format!("\n\n{}", s))
        .unwrap_or_default();

    // Determine base prompt: custom > default > empty
    let base = match custom_prompt {
        Some(custom) => custom.to_string(),
        None => options.default_prompt.unwrap_or("").to_string(),
    };

    let has_content = custom_prompt.is_some() || options.default_prompt.is_some();

    if has_content {
        let mut prompt = base;

        if !append_section.is_empty() {
            prompt.push_str(&append_section);
        }

        // Append project context files
        if !context_files.is_empty() {
            prompt.push_str("\n\n<project_context>\n\n");
            prompt.push_str("Project-specific instructions and guidelines:\n\n");
            for ctx in context_files {
                prompt.push_str(&format!(
                    "<project_instructions path=\"{}\">\n{}\n</project_instructions>\n\n",
                    ctx.path, ctx.content
                ));
            }
            prompt.push_str("</project_context>\n");
        }

        // Append skills section if read tool is available
        let has_read = selected_tools
            .map(|t| t.iter().any(|t| t == "read"))
            .unwrap_or(true);
        if has_read && !skills.is_empty() {
            prompt.push_str(&format_skills_for_prompt(skills));
        }

        prompt.push_str(&format!("\nCurrent date: {}", date));
        prompt.push_str(&format!("\nCurrent working directory: {}", prompt_cwd));
        prompt.push_str(&format!(
            "\nPlatform: {} / {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));

        // Add agent mode indicator
        if let Some(mode) = agent_mode {
            prompt.push_str(&format!("\nAgent mode: {}", mode));
        }

        prompt
    } else if !append_section.is_empty() {
        // No base content but has append — return just append
        append_section.trim_start().to_string()
    } else {
        String::new()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Build a tool snippets map from a list of AgentTools
pub fn build_tool_snippets(tools: &[AgentTool]) -> HashMap<String, String> {
    tools
        .iter()
        .map(|t| {
            let snippet = t
                .prompt_snippet
                .clone()
                .unwrap_or_else(|| t.description.clone());
            (t.name.clone(), snippet)
        })
        .collect()
}

/// Extract tool names from a list of AgentTools
pub fn build_tool_names(tools: &[AgentTool]) -> Vec<String> {
    tools.iter().map(|t| t.name.clone()).collect()
}

/// Convert ContextFile list to ContextFileRef slices
pub fn build_context_file_refs(files: &[ContextFile]) -> Vec<ContextFileRef<'_>> {
    files
        .iter()
        .map(|f| ContextFileRef {
            path: &f.path,
            content: &f.content,
        })
        .collect()
}

// ============================================================================
// Convenience Wrappers
// ============================================================================

/// Build system prompt with defaults for tools, guidelines, and context files.
/// Does NOT inject a default prompt string — use `default_prompt` in BuildSystemPromptOptions if needed.
pub fn build_system_prompt_with_defaults(
    tools: &[AgentTool],
    skills: &[Skill],
    context_files: &[ContextFile],
    custom_prompt: Option<&str>,
    append_system_prompt: Option<&str>,
    cwd: &Path,
) -> String {
    build_system_prompt_with_defaults_and_mode(
        tools,
        skills,
        context_files,
        custom_prompt,
        append_system_prompt,
        cwd,
        None,
    )
}

/// Build system prompt with defaults and agent mode
pub fn build_system_prompt_with_defaults_and_mode(
    tools: &[AgentTool],
    skills: &[Skill],
    context_files: &[ContextFile],
    custom_prompt: Option<&str>,
    append_system_prompt: Option<&str>,
    cwd: &Path,
    agent_mode: Option<&str>,
) -> String {
    let selected_tools = build_tool_names(tools);
    let tool_snippets = build_tool_snippets(tools);
    let context_refs: Vec<ContextFileRef> = build_context_file_refs(context_files);

    // Collect tool-based guidelines: per-tool guidelines + global
    let mut prompt_guidelines: Vec<String> = Vec::new();
    let has_bash = selected_tools.iter().any(|t| t == "bash");
    let has_grep = selected_tools.iter().any(|t| t == "grep");
    let has_find = selected_tools.iter().any(|t| t == "find");
    let has_ls = selected_tools.iter().any(|t| t == "ls");
    if has_bash && (has_grep || has_find || has_ls) {
        prompt_guidelines.push(
            "Prefer grep/find/ls tools over bash for file exploration (faster, respects .gitignore)"
                .to_string(),
        );
    }

    // Collect per-tool prompt_guidelines from active tools
    for tool in tools {
        for g in &tool.prompt_guidelines {
            if !prompt_guidelines.iter().any(|existing| existing == g) {
                prompt_guidelines.push(g.clone());
            }
        }
    }

    build_system_prompt(BuildSystemPromptOptions {
        custom_prompt,
        default_prompt: None,
        selected_tools: Some(&selected_tools),
        tool_snippets: Some(&tool_snippets),
        prompt_guidelines: Some(&prompt_guidelines),
        append_system_prompt,
        cwd,
        context_files: Some(&context_refs),
        skills: Some(skills),
        agent_mode,
    })
}
