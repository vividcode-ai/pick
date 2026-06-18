//! Resource loader - loads extensions, skills, prompts, themes, and context files

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::config;
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

/// A loaded prompt template
#[derive(Debug, Clone)]
pub struct PromptTemplate {
    pub name: String,
    pub description: String,
    pub file_path: String,
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
    prompts: Vec<PromptTemplate>,
    prompt_diagnostics: Vec<ResourceDiagnostic>,
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
    pub no_prompt_templates: bool,
    pub no_themes: bool,
    pub no_context_files: bool,
    pub prompt_template_paths: Vec<PathBuf>,
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
            prompts: Vec::new(),
            prompt_diagnostics: Vec::new(),
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

        // Load prompts (unless disabled)
        if !options.no_prompt_templates {
            let prompt_result =
                load_prompts(&self.agent_dir, &self.cwd, &options.prompt_template_paths);
            self.prompts = prompt_result.prompts;
            self.prompt_diagnostics = prompt_result.diagnostics;
        }

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

    pub fn prompts(&self) -> &[PromptTemplate] {
        &self.prompts
    }

    pub fn prompt_diagnostics(&self) -> &[ResourceDiagnostic] {
        &self.prompt_diagnostics
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
    if let Some(ctx) = global_context {
        if seen_paths.insert(canonicalize_path(Path::new(&ctx.path))) {
            context_files.push(ctx);
        }
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
// Prompt Templates
// ============================================================================

fn load_prompts(agent_dir: &Path, cwd: &Path, extra_paths: &[PathBuf]) -> PromptLoadResult {
    let mut prompts = Vec::new();
    let mut diagnostics = Vec::new();
    let mut seen_names = HashSet::new();

    // Load from agent dir
    let prompts_dir = agent_dir.join("prompts");
    if prompts_dir.exists() {
        load_prompts_from_dir(
            &prompts_dir,
            &mut prompts,
            &mut diagnostics,
            &mut seen_names,
        );
    }

    // Load from project dir
    let project_prompts_dir = cwd.join(config::CONFIG_DIR_NAME).join("prompts");
    if project_prompts_dir.exists() {
        load_prompts_from_dir(
            &project_prompts_dir,
            &mut prompts,
            &mut diagnostics,
            &mut seen_names,
        );
    }

    // Load from extra paths
    for path in extra_paths {
        if path.is_dir() {
            load_prompts_from_dir(path, &mut prompts, &mut diagnostics, &mut seen_names);
        } else if path.is_file() {
            load_prompt_from_file(path, &mut prompts, &mut diagnostics, &mut seen_names);
        }
    }

    PromptLoadResult {
        prompts,
        diagnostics,
    }
}

fn load_prompts_from_dir(
    dir: &Path,
    prompts: &mut Vec<PromptTemplate>,
    diagnostics: &mut Vec<ResourceDiagnostic>,
    seen_names: &mut HashSet<String>,
) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                load_prompt_from_file(&path, prompts, diagnostics, seen_names);
            }
        }
    }
}

fn load_prompt_from_file(
    path: &Path,
    prompts: &mut Vec<PromptTemplate>,
    diagnostics: &mut Vec<ResourceDiagnostic>,
    seen_names: &mut HashSet<String>,
) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            diagnostics.push(ResourceDiagnostic {
                r#type: "warning".to_string(),
                message: format!("Failed to read prompt file: {}", e),
                path: path.to_string_lossy().to_string(),
                collision: None,
            });
            return;
        }
    };

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unnamed".to_string());

    let description = parse_frontmatter_description(&content);

    if seen_names.contains(&name) {
        diagnostics.push(ResourceDiagnostic {
            r#type: "collision".to_string(),
            message: format!("Duplicate prompt name: {}", name),
            path: path.to_string_lossy().to_string(),
            collision: None,
        });
        return;
    }

    seen_names.insert(name.clone());
    prompts.push(PromptTemplate {
        name,
        description,
        file_path: path.to_string_lossy().to_string(),
    });
}

fn parse_frontmatter_description(content: &str) -> String {
    if let Some(fm) = content.strip_prefix("---") {
        if let Some(end) = fm.find("\n---") {
            let header = &fm[..end];
            for line in header.lines() {
                if let Some(desc) = line.trim().strip_prefix("description:") {
                    return desc.trim().to_string();
                }
            }
        }
    }
    String::new()
}

struct PromptLoadResult {
    prompts: Vec<PromptTemplate>,
    diagnostics: Vec<ResourceDiagnostic>,
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
        } else if path.is_file() && path.extension().map(|e| e == "json").unwrap_or(false) {
            if let Ok(t) = theme::load_theme_from_path(&path.to_string_lossy(), None) {
                let name = t.name.clone().unwrap_or_else(|| "unnamed".to_string());
                if seen_names.insert(name) {
                    themes.push(t);
                }
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
    if project_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&project_path) {
            prompts.push(content);
        }
    }

    let global_path = agent_dir.join("APPEND_SYSTEM.md");
    if global_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&global_path) {
            prompts.push(content);
        }
    }

    prompts
}
