//! Skills loading and management

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::config::CONFIG_DIR_NAME;
use crate::core::diagnostics::{ResourceCollision, ResourceDiagnostic};
use crate::core::source_info::{SourceInfo, SyntheticSourceOptions, create_synthetic_source_info};

const MAX_NAME_LENGTH: usize = 64;
const MAX_DESCRIPTION_LENGTH: usize = 1024;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub file_path: String,
    pub base_dir: String,
    pub source_info: SourceInfo,
    pub disable_model_invocation: bool,
}

#[derive(Debug, Clone)]
pub struct LoadSkillsResult {
    pub skills: Vec<Skill>,
    pub diagnostics: Vec<ResourceDiagnostic>,
}

// ============================================================================
// Frontmatter parsing for skills
// ============================================================================

fn parse_skill_frontmatter(content: &str) -> (HashMap<String, String>, String) {
    use crate::core::prompt_templates::parse_frontmatter;
    parse_frontmatter(content)
}

// ============================================================================
// Validation
// ============================================================================

fn validate_name(name: &str) -> Vec<String> {
    let mut errors = Vec::new();
    if name.len() > MAX_NAME_LENGTH {
        errors.push(format!(
            "name exceeds {} characters ({})",
            MAX_NAME_LENGTH,
            name.len()
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        errors.push(
            "name contains invalid characters (must be lowercase a-z, 0-9, hyphens only)"
                .to_string(),
        );
    }
    if name.starts_with('-') || name.ends_with('-') {
        errors.push("name must not start or end with a hyphen".to_string());
    }
    if name.contains("--") {
        errors.push("name must not contain consecutive hyphens".to_string());
    }
    errors
}

fn validate_description(description: Option<&str>) -> Vec<String> {
    let mut errors = Vec::new();
    match description {
        Some(d) if !d.trim().is_empty() => {
            if d.len() > MAX_DESCRIPTION_LENGTH {
                errors.push(format!(
                    "description exceeds {} characters ({})",
                    MAX_DESCRIPTION_LENGTH,
                    d.len()
                ));
            }
        }
        _ => errors.push("description is required".to_string()),
    }
    errors
}

// ============================================================================
// Source info creation
// ============================================================================

fn create_skill_source_info(file_path: &str, base_dir: &str, source: &str) -> SourceInfo {
    use crate::core::source_info::SourceScope;
    let scope = match source {
        "user" => Some(SourceScope::User),
        "project" => Some(SourceScope::Project),
        _ => None,
    };
    create_synthetic_source_info(
        file_path,
        SyntheticSourceOptions {
            source: "local".to_string(),
            scope,
            origin: None,
            base_dir: Some(base_dir.to_string()),
        },
    )
}

// ============================================================================
// Ignore file handling (simplified)
// ============================================================================

const IGNORE_FILE_NAMES: [&str; 3] = [".gitignore", ".ignore", ".fdignore"];

struct SkillIgnoreMatcher {
    patterns: Vec<(String, bool)>,
}

impl SkillIgnoreMatcher {
    fn new() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    fn add(&mut self, patterns: &[String]) {
        for p in patterns {
            let negated = p.starts_with('!');
            let pattern = if negated { &p[1..] } else { p };
            self.patterns.push((pattern.to_string(), negated));
        }
    }

    fn ignores(&self, path: &str) -> bool {
        let posix = path.replace('\\', "/");
        let mut ignored = false;
        for (pattern, is_negation) in &self.patterns {
            if *is_negation {
                if simple_glob_match(&posix, pattern) {
                    ignored = false;
                }
            } else if simple_glob_match(&posix, pattern) {
                ignored = true;
            }
        }
        ignored
    }
}

fn add_ignore_rules(ig: &mut SkillIgnoreMatcher, dir: &Path, root_dir: &Path) {
    let rel = pathdiff::diff_paths(dir, root_dir).unwrap_or_else(|| dir.to_path_buf());
    let prefix = if rel.to_string_lossy().is_empty() {
        String::new()
    } else {
        format!("{}/", rel.to_string_lossy().replace('\\', "/"))
    };

    for filename in &IGNORE_FILE_NAMES {
        let ignore_path = dir.join(filename);
        if !ignore_path.exists() {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&ignore_path) {
            let patterns: Vec<String> = content
                .split(['\r', '\n'])
                .filter_map(|line| prefix_ignore_pattern(line, &prefix))
                .collect();
            if !patterns.is_empty() {
                ig.add(&patterns);
            }
        }
    }
}

fn prefix_ignore_pattern(line: &str, prefix: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || (trimmed.starts_with('#') && !trimmed.starts_with("\\#")) {
        return None;
    }

    let mut pattern = trimmed.to_string();
    let negated = pattern.starts_with('!');

    if negated {
        let rest = if pattern[1..].starts_with('/') {
            &pattern[2..]
        } else {
            &pattern[1..]
        };
        let prefixed = if prefix.is_empty() {
            rest.to_string()
        } else {
            format!("{}{}", prefix, rest)
        };
        return Some(format!("!{}", prefixed));
    }

    if pattern.starts_with('\\') {
        pattern = pattern[1..].to_string();
    }
    if pattern.starts_with('/') {
        pattern = pattern[1..].to_string();
    }
    let result = if prefix.is_empty() {
        pattern
    } else {
        format!("{}{}", prefix, pattern)
    };
    Some(result)
}

fn simple_glob_match(text: &str, pattern: &str) -> bool {
    if text == pattern {
        return true;
    }
    if pattern == "*" {
        return true;
    }
    let re_str = format!("^{}$", regex::escape(pattern).replace("\\*", ".*"));
    regex::Regex::new(&re_str)
        .map(|re| re.is_match(text))
        .unwrap_or(false)
}

// ============================================================================
// Load skill from file
// ============================================================================

fn load_skill_from_file(file_path: &str, source: &str) -> (Option<Skill>, Vec<ResourceDiagnostic>) {
    let mut diagnostics = Vec::new();
    let content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => {
            diagnostics.push(ResourceDiagnostic::warning(
                e.to_string(),
                Some(file_path.to_string()),
            ));
            return (None, diagnostics);
        }
    };

    let (frontmatter, _body) = parse_skill_frontmatter(&content);
    let skill_dir = Path::new(file_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let parent_dir_name = Path::new(&skill_dir)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let description = frontmatter.get("description").cloned();

    let desc_errors = validate_description(description.as_deref());
    for error in desc_errors {
        diagnostics.push(ResourceDiagnostic::warning(
            error,
            Some(file_path.to_string()),
        ));
    }

    let name = frontmatter.get("name").cloned().unwrap_or(parent_dir_name);

    let name_errors = validate_name(&name);
    for error in name_errors {
        diagnostics.push(ResourceDiagnostic::warning(
            error,
            Some(file_path.to_string()),
        ));
    }

    let desc = match description {
        Some(ref d) if !d.trim().is_empty() => d.clone(),
        _ => return (None, diagnostics),
    };

    let disable = frontmatter
        .get("disable-model-invocation")
        .map(|v| v == "true")
        .unwrap_or(false);

    (
        Some(Skill {
            name,
            description: desc,
            file_path: file_path.to_string(),
            base_dir: skill_dir.clone(),
            source_info: create_skill_source_info(file_path, &skill_dir, source),
            disable_model_invocation: disable,
        }),
        diagnostics,
    )
}

// ============================================================================
// Load skills from directory
// ============================================================================

fn load_single_skill_from_path(
    full_path: &Path,
    source: &str,
    skills: &mut Vec<Skill>,
    diagnostics: &mut Vec<ResourceDiagnostic>,
) {
    let (skill, diags) = load_skill_from_file(&full_path.to_string_lossy(), source);
    if let Some(s) = skill {
        skills.push(s);
    }
    diagnostics.extend(diags);
}

fn load_skills_from_dir_internal(
    dir: &Path,
    source: &str,
    include_root_files: bool,
    ignore_matcher: Option<&mut SkillIgnoreMatcher>,
    root_dir: Option<&Path>,
) -> LoadSkillsResult {
    let mut skills = Vec::new();
    let mut diagnostics = Vec::new();

    if !dir.exists() {
        return LoadSkillsResult {
            skills,
            diagnostics,
        };
    }

    let root = root_dir.unwrap_or(dir);
    let mut local_ig = SkillIgnoreMatcher::new();
    let ig = match ignore_matcher {
        Some(im) => im,
        None => {
            add_ignore_rules(&mut local_ig, dir, root);
            &mut local_ig
        }
    };

    let entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(e) => e.flatten().collect(),
        Err(_) => {
            return LoadSkillsResult {
                skills,
                diagnostics,
            };
        }
    };

    // First pass: look for SKILL.md
    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if name != "SKILL.md" {
            continue;
        }

        let full_path = entry.path();
        let is_file = entry.metadata().map(|m| m.is_file()).unwrap_or(false)
            || (entry
                .metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
                && full_path.is_file());

        if !is_file {
            continue;
        }

        let rel_path = pathdiff::diff_paths(&full_path, root)
            .unwrap_or_else(|| full_path.clone())
            .to_string_lossy()
            .to_string()
            .replace('\\', "/");

        if ig.ignores(&rel_path) {
            continue;
        }

        load_single_skill_from_path(&full_path, source, &mut skills, &mut diagnostics);
        return LoadSkillsResult {
            skills,
            diagnostics,
        };
    }

    // Second pass: recurse into subdirectories and load root .md files
    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" {
            continue;
        }

        let full_path = entry.path();
        let is_dir = entry.metadata().map(|m| m.is_dir()).unwrap_or(false)
            || (entry
                .metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
                && full_path.is_dir());
        let is_file = entry.metadata().map(|m| m.is_file()).unwrap_or(false)
            || (entry
                .metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
                && full_path.is_file());

        let rel_path = pathdiff::diff_paths(&full_path, root)
            .unwrap_or_else(|| full_path.clone())
            .to_string_lossy()
            .to_string()
            .replace('\\', "/");

        let ignore_path = if is_dir {
            format!("{}/", rel_path)
        } else {
            rel_path.clone()
        };
        if ig.ignores(&ignore_path) {
            continue;
        }

        if is_dir {
            let sub =
                load_skills_from_dir_internal(&full_path, source, false, Some(ig), Some(root));
            skills.extend(sub.skills);
            diagnostics.extend(sub.diagnostics);
        } else if is_file && include_root_files && name.ends_with(".md") {
            load_single_skill_from_path(&full_path, source, &mut skills, &mut diagnostics);
        }
    }

    LoadSkillsResult {
        skills,
        diagnostics,
    }
}

// ============================================================================
// formatSkillsForPrompt
// ============================================================================

/// Format skills for inclusion in a system prompt
pub fn format_skills_for_prompt(skills: &[Skill]) -> String {
    let visible: Vec<&Skill> = skills
        .iter()
        .filter(|s| !s.disable_model_invocation)
        .collect();
    if visible.is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();
    lines.push(
        "\n\nThe following skills provide specialized instructions for specific tasks.".to_string(),
    );
    lines.push(
        "Use the read tool to load a skill's file when the task matches its description."
            .to_string(),
    );
    lines.push("When a skill file references a relative path, resolve it against the skill directory (parent of SKILL.md / dirname of the path) and use that absolute path in tool commands.".to_string());
    lines.push(String::new());
    lines.push("<available_skills>".to_string());

    for skill in visible {
        lines.push("  <skill>".to_string());
        lines.push(format!("    <name>{}</name>", escape_xml(&skill.name)));
        lines.push(format!(
            "    <description>{}</description>",
            escape_xml(&skill.description)
        ));
        lines.push(format!(
            "    <location>{}</location>",
            escape_xml(&skill.file_path)
        ));
        lines.push("  </skill>".to_string());
    }

    lines.push("</available_skills>".to_string());
    lines.join("\n")
}

fn escape_xml(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&apos;".to_string(),
            _ => c.to_string(),
        })
        .collect()
}

// ============================================================================
// LoadSkills (main entry point)
// ============================================================================

pub struct LoadSkillsOptions {
    pub cwd: String,
    pub agent_dir: String,
    pub skill_paths: Vec<String>,
    pub include_defaults: bool,
}

/// Add skills from a LoadSkillsResult into the maps, handling dedup and collisions
fn add_skills_inner(
    skill_map: &mut HashMap<String, Skill>,
    real_path_set: &mut HashSet<String>,
    diagnostics: &mut Vec<ResourceDiagnostic>,
    result: LoadSkillsResult,
) {
    diagnostics.extend(result.diagnostics);
    for skill in result.skills {
        let real_path = std::path::absolute(Path::new(&skill.file_path))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| skill.file_path.clone());

        if real_path_set.contains(&real_path) {
            continue;
        }

        if let Some(existing) = skill_map.get(&skill.name) {
            diagnostics.push(ResourceDiagnostic::collision(ResourceCollision {
                resource_type: "skill".to_string(),
                name: skill.name.clone(),
                winner_path: existing.file_path.clone(),
                loser_path: skill.file_path.clone(),
                winner_source: None,
                loser_source: None,
            }));
        } else {
            real_path_set.insert(real_path);
            skill_map.insert(skill.name.clone(), skill);
        }
    }
}

/// Load skills from all configured locations
pub fn load_skills(options: LoadSkillsOptions) -> LoadSkillsResult {
    let agent_dir = &options.agent_dir;
    let cwd = &options.cwd;
    let skill_paths = &options.skill_paths;
    let include_defaults = options.include_defaults;

    let mut skill_map: HashMap<String, Skill> = HashMap::new();
    let mut real_path_set: HashSet<String> = HashSet::new();
    let mut all_diagnostics: Vec<ResourceDiagnostic> = Vec::new();

    if include_defaults {
        add_skills_inner(
            &mut skill_map,
            &mut real_path_set,
            &mut all_diagnostics,
            load_skills_from_dir_internal(
                &Path::new(agent_dir).join("skills"),
                "user",
                true,
                None,
                None,
            ),
        );
        add_skills_inner(
            &mut skill_map,
            &mut real_path_set,
            &mut all_diagnostics,
            load_skills_from_dir_internal(
                &Path::new(cwd).join(CONFIG_DIR_NAME).join("skills"),
                "project",
                true,
                None,
                None,
            ),
        );
    }

    let user_skills_dir = Path::new(agent_dir).join("skills");
    let project_skills_dir = Path::new(cwd).join(CONFIG_DIR_NAME).join("skills");

    let is_under_path = |target: &str, root: &str| -> bool {
        let normalized_root = std::path::absolute(Path::new(root))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| root.to_string());
        let normalized_target = std::path::absolute(Path::new(target))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| target.to_string());
        normalized_target == normalized_root
            || normalized_target.starts_with(&format!("{}/", normalized_root))
    };

    let get_source = |resolved_path: &str| -> &str {
        if !include_defaults {
            if is_under_path(resolved_path, &user_skills_dir.to_string_lossy()) {
                return "user";
            }
            if is_under_path(resolved_path, &project_skills_dir.to_string_lossy()) {
                return "project";
            }
        }
        "path"
    };

    for raw_path in skill_paths {
        let resolved = if Path::new(raw_path).is_absolute() {
            raw_path.clone()
        } else {
            Path::new(cwd).join(raw_path).to_string_lossy().to_string()
        };

        let resolved_path = Path::new(&resolved);
        if !resolved_path.exists() {
            all_diagnostics.push(ResourceDiagnostic::warning(
                "skill path does not exist".to_string(),
                Some(resolved),
            ));
            continue;
        }

        let source = get_source(&resolved);

        if resolved_path.is_dir() {
            add_skills_inner(
                &mut skill_map,
                &mut real_path_set,
                &mut all_diagnostics,
                load_skills_from_dir_internal(resolved_path, source, true, None, None),
            );
        } else if resolved_path.is_file() && resolved.ends_with(".md") {
            let (skill, diags) = load_skill_from_file(&resolved, source);
            if let Some(s) = skill {
                add_skills_inner(
                    &mut skill_map,
                    &mut real_path_set,
                    &mut all_diagnostics,
                    LoadSkillsResult {
                        skills: vec![s],
                        diagnostics: diags,
                    },
                );
            } else {
                all_diagnostics.extend(diags);
            }
        } else {
            all_diagnostics.push(ResourceDiagnostic::warning(
                "skill path is not a markdown file".to_string(),
                Some(resolved),
            ));
        }
    }

    LoadSkillsResult {
        skills: skill_map.into_values().collect(),
        diagnostics: all_diagnostics,
    }
}
