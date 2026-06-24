//! System prompt construction and project context loading

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use crate::config::{get_docs_path, get_examples_path, get_readme_path};
use crate::core::agent_mode::AgentMode;
use crate::core::resource_loader::ContextFile;
use pick_agent::core::state::AgentTool;
use pick_agent::skills::{Skill, format_skills_for_prompt};

/// Options for building a system prompt
pub struct BuildSystemPromptOptions<'a> {
    /// Custom system prompt (replaces default)
    pub custom_prompt: Option<&'a str>,
    /// Tools to include in prompt
    pub selected_tools: Option<&'a [String]>,
    /// One-line tool snippets keyed by tool name
    pub tool_snippets: Option<&'a std::collections::HashMap<String, String>>,
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
    /// Agent mode (build/plan) for mode-specific prompt injection
    pub agent_mode: Option<&'a AgentMode>,
}

/// Reference to a context file for the system prompt
pub struct ContextFileRef<'a> {
    pub path: &'a str,
    pub content: &'a str,
}

/// Build the system prompt with tools, guidelines, and context
pub fn build_system_prompt(options: BuildSystemPromptOptions) -> String {
    let custom_prompt = options.custom_prompt;
    let selected_tools = options.selected_tools;
    let tool_snippets = options.tool_snippets;
    let prompt_guidelines = options.prompt_guidelines;
    let append_system_prompt = options.append_system_prompt;
    let cwd = options.cwd;
    let context_files = options.context_files.unwrap_or(&[]);
    let skills = options.skills.unwrap_or(&[]);

    let prompt_cwd = cwd.to_string_lossy().replace('\\', "/");

    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();

    let append_section = append_system_prompt
        .filter(|s| !s.is_empty())
        .map(|s| format!("\n\n{}", s))
        .unwrap_or_default();

    if let Some(custom) = custom_prompt {
        let mut prompt = custom.to_string();

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

        // Add agent mode indicator so LLM always knows its current mode
        if let Some(mode) = options.agent_mode {
            prompt.push_str(&format!("\nAgent mode: {}", mode));
        }

        return prompt;
    }

    // Default system prompt
    let readme_path = get_readme_path().to_string_lossy().to_string();
    let docs_path = get_docs_path().to_string_lossy().to_string();
    let examples_path = get_examples_path().to_string_lossy().to_string();

    // Build tools list
    let default_tools = ["read", "bash", "edit", "write"];
    let default_tools_owned: Vec<String> = default_tools.iter().map(|s| s.to_string()).collect();
    let tools = selected_tools.unwrap_or(&default_tools_owned);

    let visible_tools: Vec<&String> = tools
        .iter()
        .filter(|name| tool_snippets.is_some_and(|s| s.contains_key(*name)))
        .collect();

    let tools_list = if visible_tools.is_empty() {
        "(none)".to_string()
    } else {
        visible_tools
            .iter()
            .map(|name| {
                let snippet = tool_snippets
                    .and_then(|s| s.get(*name))
                    .map(|s| s.as_str())
                    .unwrap_or("");
                format!("- {}: {}", name, snippet)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    // Build guidelines
    let mut guidelines_list: Vec<String> = Vec::new();
    let mut guidelines_set: HashSet<String> = HashSet::new();
    let mut add_guideline = |guideline: &str| {
        if guidelines_set.insert(guideline.to_string()) {
            guidelines_list.push(guideline.to_string());
        }
    };

    let has_bash = tools.iter().any(|t| t == "bash");
    let has_grep = tools.iter().any(|t| t == "grep");
    let has_find = tools.iter().any(|t| t == "find");
    let has_ls = tools.iter().any(|t| t == "ls");
    let has_read = tools.iter().any(|t| t == "read");

    // File exploration guidelines
    if has_bash && !has_grep && !has_find && !has_ls {
        add_guideline("Use bash for file operations like ls, rg, find");
    } else if has_bash && (has_grep || has_find || has_ls) {
        add_guideline(
            "Prefer grep/find/ls tools over bash for file exploration (faster, respects .gitignore)",
        );
    }

    if let Some(extra_guidelines) = prompt_guidelines {
        for g in extra_guidelines {
            let normalized = g.trim();
            if !normalized.is_empty() {
                add_guideline(normalized);
            }
        }
    }

    add_guideline("Be concise in your responses");
    add_guideline("Show file paths clearly when working with files");

    let guidelines = guidelines_list
        .iter()
        .map(|g| format!("- {}", g))
        .collect::<Vec<_>>()
        .join("\n");

    let mut prompt = format!(
        r#"You are an expert-level programming assistant Pick, a coding agent harness. You help users by reading files, executing commands, editing code, and writing new files.

Available tools:
{}

In addition to the tools above, you may have access to other custom tools depending on the project.

Guidelines:
{}

**Bullets**
- Use `-` followed by a space for every bullet.
- Merge related points when possible; avoid a bullet for every trivial detail.
- Keep bullets to one line unless breaking for clarity is unavoidable.
- Group into short lists (4-6 bullets) ordered by importance.
- Use consistent keyword phrasing and formatting across sections.

**Monospace**
- Wrap commands, file paths, env vars, and code identifiers in backticks (\`...\`).
- Never mix monospace and bold markers; choose one based on whether it's a keyword (**) or inline code/path (\`).
- Multi-line code samples should be wrapped in fenced code blocks with a language identifier.

**File References**
- Use \`path/to/file\` to make file paths clickable.
- Format: \`path/file:line\` or \`path/file#Lline\` (1-based, column defaults to 1).
- Do not use URIs like file:// or vscode://. Do not provide range of lines.

**Style**
- Be concise and factual; avoid filler or commentary.
- Use present tense and active voice.
- Keep descriptions self-contained; don't refer to "above" or "below".
- Use parallel structure in lists for consistency.

**Don't**
- Don't nest bullets or create deep hierarchies.
- Don't output ANSI escape codes directly.
- Don't cram unrelated keywords into a single bullet; split for clarity.
- Don't let keyword lists run long.

**Verbosity**
- Tiny/small change (<= ~10 lines): 2-5 sentences or <=3 bullets. No headings.
- Medium change (single area or a few files): <=6 bullets or 6-10 sentences.
- Large/multi-file change: Summarize per file with 1-2 bullets.
- Never include before/after pairs or full method bodies in the final message.

**Tool Calls**
- Each tool has required and optional parameters. Always provide all required parameters when calling a tool.
- If a tool returns an error, read the error message carefully. It tells you exactly which parameter is missing or what went wrong.
- Fix the issue before retrying. Do not call the same tool repeatedly with the same incorrect parameters.
- If you cannot resolve a tool error after fixing parameters, provide your best answer directly without using tools.

Pick documentation (read only when the user asks about Pick itself, its SDK, extensions, themes, skills, or TUI):
- Main documentation: {}
- Additional docs: {}
- Examples: {} (extensions, custom tools, SDK)
- When reading Pick docs or examples, resolve docs/... under Additional docs and examples/... under Examples, not the current working directory
- When asked about: extensions (docs/extensions.md), themes (docs/themes.md), skills (docs/skills.md), TUI components (docs/tui.md), keybindings (docs/keybindings.md), SDK integrations (docs/sdk.md), custom providers (docs/custom-provider.md), adding models (docs/models.md), Pick packages (docs/packages.md)
- When working on Pick topics, read the docs and examples, and follow .md cross-references before implementing
- Always read Pick .md files completely and follow links to related docs (e.g., tui.md for TUI API details)"#,
        tools_list, guidelines, readme_path, docs_path, examples_path,
    );

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
    if has_read && !skills.is_empty() {
        prompt.push_str(&format_skills_for_prompt(skills));
    }

    // Add date, working directory, and platform last
    prompt.push_str(&format!("\nCurrent date: {}", date));
    prompt.push_str(&format!("\nCurrent working directory: {}", prompt_cwd));
    prompt.push_str(&format!(
        "\nPlatform: {} / {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));

    // Add agent mode indicator so LLM always knows its current mode
    if let Some(mode) = options.agent_mode {
        prompt.push_str(&format!("\nAgent mode: {}", mode));
    }

    prompt
}

/// Build a tool snippets map from a list of AgentTools.
/// Maps tool name → one-line description for inclusion in the system prompt.
/// Uses prompt_snippet if available, falling back to description.
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

/// Extract tool names from a list of AgentTools.
pub fn build_tool_names(tools: &[AgentTool]) -> Vec<String> {
    tools.iter().map(|t| t.name.clone()).collect()
}

/// Convert ContextFile list (from ResourceLoader) to the ContextFileRef slices
/// needed by BuildSystemPromptOptions.
pub fn build_context_file_refs(files: &[ContextFile]) -> Vec<ContextFileRef<'_>> {
    files
        .iter()
        .map(|f| ContextFileRef {
            path: &f.path,
            content: &f.content,
        })
        .collect()
}

/// Convenience wrapper that builds a system prompt from the most common inputs.
/// Internally constructs tool snippets, context file refs, and guidelines.
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
    agent_mode: Option<&AgentMode>,
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
