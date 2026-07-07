//! CLI-specific system prompt construction — delegates to pick-agent for core logic

use std::collections::HashMap;
use std::path::Path;

use crate::config::{get_docs_path, get_examples_path, get_readme_path};
use crate::core::agent_mode::AgentMode;
use pick_agent::skills::{Skill, format_skills_for_prompt};
use pick_agent::system_prompt::{ContextFile, ContextFileRef};
use pick_agent::system_prompt::{build_context_file_refs, build_tool_names, build_tool_snippets};

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

/// Build the CLI default prompt string
fn build_cli_default_prompt(
    selected_tools: Option<&[String]>,
    tool_snippets: Option<&HashMap<String, String>>,
    prompt_guidelines: Option<&[String]>,
    tool_usage_examples: Option<&HashMap<String, Vec<String>>>,
    cwd: &Path,
    context_files: &[ContextFileRef],
    skills: &[Skill],
    agent_mode: Option<&str>,
) -> String {
    let prompt_cwd = cwd.to_string_lossy().replace('\\', "/");

    let now = chrono::Local::now();
    let date = now.format("%Y-%m-%d").to_string();

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

    let has_bash = tools.iter().any(|t| t == "bash");
    let has_grep = tools.iter().any(|t| t == "grep");
    let has_find = tools.iter().any(|t| t == "find");
    let has_ls = tools.iter().any(|t| t == "ls");
    let has_read = tools.iter().any(|t| t == "read");

    let readme_path = get_readme_path().to_string_lossy().to_string();
    let docs_path = get_docs_path().to_string_lossy().to_string();
    let examples_path = get_examples_path().to_string_lossy().to_string();

    // Compute file exploration hints
    let mut extra_guidelines: Vec<String> = Vec::new();
    if has_bash && !has_grep && !has_find && !has_ls {
        extra_guidelines.push("Use bash for file operations like ls, rg, find".to_string());
    } else if has_bash && (has_grep || has_find || has_ls) {
        extra_guidelines.push(
            "Prefer grep/find/ls tools over bash for file exploration (faster, respects .gitignore)"
                .to_string(),
        );
    }
    // Add parallelization hint
    extra_guidelines.push(
        "Parallelize independent tool calls when possible (e.g., read multiple files simultaneously)".to_string(),
    );

    let mut prompt = format!(
        r#"You are an expert-level programming assistant Pick, a coding agent harness. You help users by reading files, executing commands, editing code, and writing new files.

Available tools:
{}

In addition to the tools above, you may have access to other custom tools depending on the project."#,
        tools_list,
    );

    // ===== How you work section =====
    prompt.push_str(
        r#"

# How you work

## Personality
Be concise, direct, and friendly. Pick operates in two modes: Build (full read/write) and Plan (read-only research). Adapt your tone to the task — surgical precision for existing code, creativity for new work. Always state assumptions and next steps clearly.

## AGENTS.md
Project AGENTS.md and CLAUDE.md files are automatically injected as <project_context>. They contain coding conventions, test instructions, and organization info. Follow them for files in their scope. These prompt instructions take precedence over AGENTS.md. Do not re-read them.

## Responsiveness
Before each tool call, send a brief 1-2 sentence preamble explaining your next action. Group related actions under one preamble. Use present tense and an active, collaborative tone. Pick streams your responses and tool calls live — there's no need to narrate every keystroke.

## Planning
Pick has two operational modes:

**Plan mode** (read-only): For research, exploration, and planning. Use `plan_enter` to switch. Only read tools and safe shell commands are available. You MUST NOT edit files or run destructive commands.

**Build mode** (default): Full read/write access for implementation. Use `plan_exit` to switch back.

Use `todo_plan` to track multi-step tasks. Break work into verifiable steps. Do not pad with filler.

Use a plan when:
- The task has multiple logical phases or dependencies
- You need to explore before implementing
- The user asked for multiple things in one prompt

## Task execution
Persist until the query is resolved. Do not guess or make up answers.

Use the right tool for each job:
- **edit**: For targeted file changes with exact text replacement.
- **write**: For new files or complete rewrites.
- **subagent**: Delegate specialized or exploratory subtasks to isolated child agents. Use parallel mode for independent tasks.
- **goal**: Track progress against a defined objective.

Skills are listed in <available_skills>. Read a skill's file when a task matches its description. Follow the task-specific instructions they contain.

If a command is restricted by sandbox or policy, read the error and adjust your approach. After repeated denials the conversation will be interrupted.

Coding guidelines:
- Fix root cause, not surface symptoms
- Keep changes minimal and consistent with existing style
- Do not fix unrelated bugs — mention them in your final message
- Do not commit, add inline comments, or create branches unless asked
- NEVER use citation markers like 【file†L5-L14】 — use plain `path/file:line` references

## Validating your work
Use tests to verify your work when available. Start with the most specific test for the code you changed, then broaden. If no test exists and the codebase has a test pattern, you may add one.

Run linting or formatting if the project has a configured tool. Iterate up to 3 times on auto-formatting, then move on.

Do not fix unrelated bugs discovered during validation. Note them in your final message.

## Ambition vs. precision
For new projects with no prior code, be ambitious and creative — suggest good defaults, generate full implementations.

For existing codebases, be surgically precise. Match the surrounding style, structure, and conventions. Do not refactor unrelated code or rename symbols without explicit request.

Use judgment: creative scope for vague tasks, tight targeting for specific asks.

## Sharing progress updates
For long tasks, periodically send a 1-sentence update: what's been done, what's next. Before large operations like writing a new file, notify the user first. Let them know what you're about to do and why.

## Presenting your work
Deliver final messages like a concise teammate giving a handoff. Use a natural, conversational tone for casual queries. For substantive changes, follow the Output Format section below.

- Reference files with `path/file:line` — the user can click to open them
- Do not dump full file contents unless asked — the user sees the same filesystem
- If there's a logical next step, suggest it briefly
- Use headers only when they improve scanability"#,
    );

    // Output Format section
    prompt.push_str(
        r#"

# Output Format

**Bullets**
- Use `-` followed by a space for every bullet.
- Merge related points when possible; avoid a bullet for every trivial detail.
- Keep bullets to one line unless breaking for clarity is unavoidable.
- Group into short lists (4-6 bullets) ordered by importance.
- Use consistent keyword phrasing and formatting across sections.

**Monospace**
- Wrap commands, file paths, env vars, and code identifiers in backticks (`...`).
- Never mix monospace and bold markers; choose one based on whether it's a keyword (**) or inline code/path (`).
- Multi-line code samples should be wrapped in fenced code blocks with a language identifier.

**File References**
- Use `path/to/file` to make file paths clickable.
- Format: `path/file:line` or `path/file#Lline` (1-based, column defaults to 1).
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
- Never include before/after pairs or full method bodies in the final message."#,
    );

    // Pick documentation section
    prompt.push_str(&format!(
        r#"

Pick documentation (read only when the user asks about Pick itself, its SDK, extensions, themes, skills, or TUI):
- Main documentation: {}
- Additional docs: {}
- Examples: {} (extensions, custom tools, SDK)
- When reading Pick docs or examples, resolve docs/... under Additional docs and examples/... under Examples, not the current working directory
- When asked about: extensions (docs/extensions.md), themes (docs/themes.md), skills (docs/skills.md), TUI components (docs/tui.md), keybindings (docs/keybindings.md), SDK integrations (docs/sdk.md), custom providers (docs/custom-provider.md), adding models (docs/models.md), Pick packages (docs/packages.md)
- When working on Pick topics, read the docs and examples, and follow .md cross-references before implementing
- Always read Pick .md files completely and follow links to related docs (e.g., tui.md for TUI API details)"#,
        readme_path, docs_path, examples_path,
    ));

    // ===== Tool Guidelines section =====
    prompt.push_str("\n\n# Tool Guidelines\n\n## Tool Selection\n");
    prompt.push_str("- **Read files**: `read` for specific files; `grep` for content search; `find`/`ls` for directory listing\n");
    prompt.push_str("- **Modify files**: `edit` for targeted changes; `write` for new files or complete rewrites\n");
    prompt.push_str("- **Search**: `grep` for text patterns; `find` for filename patterns\n");
    prompt.push_str("- **Run commands**: `bash` for shell execution\n");
    prompt.push_str("- **Plan**: `todo_plan` for multi-step task tracking\n");
    prompt.push_str("- **Delegate**: `subagent` for specialized subtasks\n");
    prompt.push_str("- **Web**: `webfetch` for URL content\n");
    prompt.push_str("- **Ask**: `question` to ask the user for input\n");

    // General guidelines
    if !extra_guidelines.is_empty() {
        prompt.push_str("\n## General\n");
        for g in &extra_guidelines {
            prompt.push_str(&format!("- {}\n", g));
        }
    }

    // Per-tool guidelines + usage examples
    if let Some(examples) = tool_usage_examples {
        let mut tool_names: Vec<&String> = examples.keys().collect();
        tool_names.sort();
        for name in &tool_names {
            if let Some(examples_list) = examples.get(*name) {
                if !examples_list.is_empty() {
                    prompt.push_str(&format!("\n## {}\n", name));
                    for ex in examples_list.iter() {
                        prompt.push_str(&format!("- Usage: `{}`\n", ex));
                    }
                }
            }
        }
    }

    // Unknown/other tool guidelines: show collected guidelines as General
    if let Some(extra) = prompt_guidelines {
        if !extra.is_empty() {
            prompt.push_str("\n## General Guidelines\n");
            for g in extra {
                prompt.push_str(&format!("- {}\n", g));
            }
        }
    }

    // General tool call tips
    prompt.push_str(
        "\n## Tool Calls\n- Each tool has required and optional parameters. Always provide all required parameters when calling a tool.\n\
         - If a tool returns an error, read the error message carefully. It tells you exactly which parameter is missing or what went wrong.\n\
         - Fix the issue before retrying. Do not call the same tool repeatedly with the same incorrect parameters.\n\
         - If you cannot resolve a tool error after fixing parameters, provide your best answer directly without using tools.\n",
    );

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

    // Add agent mode indicator
    if let Some(mode) = agent_mode {
        prompt.push_str(&format!("\nAgent mode: {}", mode));
    }

    prompt
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
    let agent_mode = options.agent_mode;

    let append_section = append_system_prompt
        .filter(|s| !s.is_empty())
        .map(|s| format!("\n\n{}", s))
        .unwrap_or_default();

    if let Some(custom) = custom_prompt {
        // Custom prompt path: delegate to agent crate (no default prompt)
        return pick_agent::system_prompt::build_system_prompt(
            pick_agent::system_prompt::BuildSystemPromptOptions {
                custom_prompt: Some(custom),
                default_prompt: None,
                selected_tools,
                tool_snippets,
                prompt_guidelines,
                append_system_prompt,
                cwd,
                context_files: Some(context_files),
                skills: Some(skills),
                agent_mode: agent_mode.map(|m| m.as_str()),
            },
        );
    }

    // CLI default prompt path
    let default_prompt = build_cli_default_prompt(
        selected_tools,
        tool_snippets,
        prompt_guidelines,
        None,
        cwd,
        context_files,
        skills,
        agent_mode.map(|m| m.as_str()),
    );

    let mut result = default_prompt;
    if !append_section.is_empty() {
        result.push_str(&append_section);
    }
    result
}

/// CLI-specific convenience wrapper — delegates to agent when custom prompt is given,
/// otherwise uses CLI default prompt.
pub fn build_system_prompt_with_defaults(
    tools: &[pick_agent::core::state::AgentTool],
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

/// CLI-specific convenience wrapper with agent mode support
pub fn build_system_prompt_with_defaults_and_mode(
    tools: &[pick_agent::core::state::AgentTool],
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

    // Collect per-tool prompt_guidelines and usage_examples from active tools
    let mut prompt_guidelines: Vec<String> = Vec::new();
    let mut tool_usage_examples: HashMap<String, Vec<String>> = HashMap::new();
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
    for tool in tools {
        for g in &tool.prompt_guidelines {
            if !prompt_guidelines.iter().any(|existing| existing == g) {
                prompt_guidelines.push(g.clone());
            }
        }
        if let Some(ref examples) = tool.usage_example {
            tool_usage_examples.insert(tool.name.clone(), examples.clone());
        }
    }

    let agent_mode_str = agent_mode.map(|m| m.as_str());

    if let Some(custom) = custom_prompt {
        // Custom prompt: delegate to agent crate
        return pick_agent::system_prompt::build_system_prompt(
            pick_agent::system_prompt::BuildSystemPromptOptions {
                custom_prompt: Some(custom),
                default_prompt: None,
                selected_tools: Some(&selected_tools),
                tool_snippets: Some(&tool_snippets),
                prompt_guidelines: Some(&prompt_guidelines),
                append_system_prompt,
                cwd,
                context_files: Some(&context_refs),
                skills: Some(skills),
                agent_mode: agent_mode_str,
            },
        );
    }

    // No custom prompt: build CLI default prompt
    let default = build_cli_default_prompt(
        Some(&selected_tools),
        Some(&tool_snippets),
        Some(&prompt_guidelines),
        Some(&tool_usage_examples),
        cwd,
        &context_refs,
        skills,
        agent_mode_str,
    );

    let append_section = append_system_prompt
        .filter(|s| !s.is_empty())
        .map(|s| format!("\n\n{}", s))
        .unwrap_or_default();

    let mut result = default;
    if !append_section.is_empty() {
        result.push_str(&append_section);
    }
    result
}
