//! Resource loader - loads extensions, skills, themes, and context files

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::config;
use crate::core::prompt_templates::{self, PromptTemplate as FullPromptTemplate};
use crate::core::theme::{self, Theme};
use pick_agent::extensions::loader::discover_and_load_extensions;
use pick_agent::extensions::types::LoadExtensionsResult;
use pick_agent::skills::{self, Skill};

// ============================================================================
// Types
// ============================================================================

/// A diagnostic message during resource loading
#[derive(Debug, Clone)]
pub struct ResourceDiagnostic {
    pub r#type: String, // "error", "warning", "collision"
    pub message: String,
    pub path: String,
    pub collision: Option<ResourceCollision>,
}

#[derive(Debug, Clone)]
pub struct ResourceCollision {
    pub resource_type: String,
    pub name: String,
    pub winner_path: String,
    pub loser_path: String,
}

/// Source information for a resource
#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub path: String,
    pub source: String,
    pub scope: String,
    pub origin: String,
    pub base_dir: Option<String>,
}

/// Result from loading resources
pub struct ResourceLoader {
    // Configuration
    cwd: PathBuf,
    agent_dir: PathBuf,

    // Loaded resources
    extensions_result: Option<LoadExtensionsResult>,
    skills: Vec<Skill>,
    skill_diagnostics: Vec<ResourceDiagnostic>,
    commands: Vec<prompt_templates::PromptTemplate>,
    command_diagnostics: Vec<ResourceDiagnostic>,
    themes: Vec<Theme>,
    theme_diagnostics: Vec<ResourceDiagnostic>,
    agents_files: Vec<ContextFile>,
    system_prompt: Option<String>,
    append_system_prompt: Vec<String>,
}

/// Options to control which resources are loaded
#[derive(Debug, Clone, Default)]
pub struct ResourceLoaderOptions {
    pub no_skills: bool,
    pub no_themes: bool,
    pub no_context_files: bool,
    pub theme_paths: Vec<PathBuf>,
}

impl ResourceLoader {
    pub fn new(cwd: PathBuf, agent_dir: PathBuf) -> Self {
        Self {
            cwd,
            agent_dir,
            extensions_result: None,
            skills: Vec::new(),
            skill_diagnostics: Vec::new(),
            commands: Vec::new(),
            command_diagnostics: Vec::new(),
            themes: Vec::new(),
            theme_diagnostics: Vec::new(),
            agents_files: Vec::new(),
            system_prompt: None,
            append_system_prompt: Vec::new(),
        }
    }

    /// Reload all resources
    pub async fn reload(&mut self, ext_paths: &[String]) {
        self.reload_with_options(ext_paths, &ResourceLoaderOptions::default())
            .await;
    }

    /// Reload all resources with options to disable specific resource types
    pub async fn reload_with_options(
        &mut self,
        ext_paths: &[String],
        options: &ResourceLoaderOptions,
    ) {
        // Load extensions
        self.extensions_result =
            Some(discover_and_load_extensions(ext_paths, &self.cwd, &self.agent_dir).await);

        // Load skills (unless disabled)
        if !options.no_skills {
            let skill_result = load_skills(&self.agent_dir, &self.cwd, &[]);
            self.skills = skill_result.skills;
            self.skill_diagnostics = skill_result
                .diagnostics
                .into_iter()
                .map(|d| ResourceDiagnostic {
                    r#type: "warning".to_string(),
                    message: d.message,
                    path: d.path,
                    collision: None,
                })
                .collect();
        }

        // Load commands
        let cmds = prompt_templates::load_command_templates(&self.agent_dir, &self.cwd);
        self.commands = cmds;

        // Load themes (unless disabled)
        if !options.no_themes {
            let theme_result = load_themes(&self.agent_dir, &self.cwd, &options.theme_paths);
            self.themes = theme_result.themes;
            self.theme_diagnostics = theme_result.diagnostics;
        }

        // Load AGENTS.md / CLAUDE.md context files (unless disabled)
        if !options.no_context_files {
            self.agents_files = load_project_context_files(&self.cwd, &self.agent_dir);
        }

        // Load system prompt
        self.system_prompt = discover_system_prompt(&self.agent_dir, &self.cwd);

        // Load append system prompt
        self.append_system_prompt = discover_append_system_prompt(&self.agent_dir, &self.cwd);
    }

    pub fn extensions_result(&self) -> Option<&LoadExtensionsResult> {
        self.extensions_result.as_ref()
    }

    pub fn skills(&self) -> &[Skill] {
        &self.skills
    }

    pub fn skill_diagnostics(&self) -> &[ResourceDiagnostic] {
        &self.skill_diagnostics
    }

    pub fn commands(&self) -> &[FullPromptTemplate] {
        &self.commands
    }

    pub fn command_diagnostics(&self) -> &[ResourceDiagnostic] {
        &self.command_diagnostics
    }

    pub fn themes(&self) -> &[Theme] {
        &self.themes
    }

    pub fn theme_diagnostics(&self) -> &[ResourceDiagnostic] {
        &self.theme_diagnostics
    }

    pub fn agents_files(&self) -> &[ContextFile] {
        &self.agents_files
    }

    pub fn system_prompt(&self) -> Option<&str> {
        self.system_prompt.as_deref()
    }

    pub fn append_system_prompt(&self) -> &[String] {
        &self.append_system_prompt
    }
}

// ============================================================================
// Context Files (AGENTS.md / CLAUDE.md)
// ============================================================================

#[derive(Debug, Clone)]
pub struct ContextFile {
    pub path: String,
    pub content: String,
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

// ============================================================================
// Theme Loading
// ============================================================================

fn load_themes(agent_dir: &Path, cwd: &Path, extra_paths: &[PathBuf]) -> ThemeLoadResult {
    let mut themes = Vec::new();
    let mut diagnostics = Vec::new();
    let mut seen_names = HashSet::new();

    // Load from agent dir
    let themes_dir = agent_dir.join("themes");
    if themes_dir.exists() {
        for (name, json) in theme::load_json_themes_from_dir(&themes_dir) {
            if seen_names.insert(name.clone()) {
                themes.push(theme::create_theme_from_json(&json, None));
            } else {
                diagnostics.push(ResourceDiagnostic {
                    r#type: "collision".to_string(),
                    message: format!("Duplicate theme name: {}", name),
                    path: themes_dir
                        .join(format!("{}.json", name))
                        .to_string_lossy()
                        .to_string(),
                    collision: None,
                });
            }
        }
    }

    // Load from project dir
    let project_themes_dir = cwd.join(config::CONFIG_DIR_NAME).join("themes");
    if project_themes_dir.exists() {
        for (name, json) in theme::load_json_themes_from_dir(&project_themes_dir) {
            if seen_names.insert(name.clone()) {
                themes.push(theme::create_theme_from_json(&json, None));
            } else {
                diagnostics.push(ResourceDiagnostic {
                    r#type: "collision".to_string(),
                    message: format!("Duplicate theme name: {}", name),
                    path: project_themes_dir
                        .join(format!("{}.json", name))
                        .to_string_lossy()
                        .to_string(),
                    collision: None,
                });
            }
        }
    }

    // Load from extra paths
    for path in extra_paths {
        if path.is_dir() {
            for (name, json) in theme::load_json_themes_from_dir(path) {
                if seen_names.insert(name.clone()) {
                    themes.push(theme::create_theme_from_json(&json, None));
                }
            }
        } else if path.is_file()
            && path.extension().map(|e| e == "json").unwrap_or(false)
            && let Ok(t) = theme::load_theme_from_path(&path.to_string_lossy(), None)
        {
            let name = t.name.clone().unwrap_or_else(|| "unnamed".to_string());
            if seen_names.insert(name) {
                themes.push(t);
            }
        }
    }

    ThemeLoadResult {
        themes,
        diagnostics,
    }
}

struct ThemeLoadResult {
    themes: Vec<Theme>,
    diagnostics: Vec<ResourceDiagnostic>,
}

// ============================================================================
// Skills
// ============================================================================

fn load_skills(agent_dir: &Path, cwd: &Path, extra_paths: &[PathBuf]) -> SkillLoadResult {
    let result = skills::load_skills(agent_dir, cwd, extra_paths);
    SkillLoadResult {
        skills: result.skills,
        diagnostics: result
            .diagnostics
            .into_iter()
            .map(|d| ResourceDiagnostic {
                r#type: "warning".to_string(),
                message: d.message,
                path: d.path,
                collision: None,
            })
            .collect(),
    }
}

struct SkillLoadResult {
    skills: Vec<Skill>,
    diagnostics: Vec<ResourceDiagnostic>,
}

// ============================================================================
// System Prompt Discovery
// ============================================================================

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

fn discover_system_prompt(agent_dir: &Path, cwd: &Path) -> Option<String> {
    let project_path = cwd.join(config::CONFIG_DIR_NAME).join("SYSTEM.md");
    if project_path.exists() {
        return std::fs::read_to_string(&project_path).ok();
    }

    let global_path = agent_dir.join("SYSTEM.md");
    if global_path.exists() {
        return std::fs::read_to_string(&global_path).ok();
    }

    None
}

fn discover_append_system_prompt(agent_dir: &Path, cwd: &Path) -> Vec<String> {
    let mut prompts = Vec::new();

    let project_path = cwd.join(config::CONFIG_DIR_NAME).join("APPEND_SYSTEM.md");
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
