//! Skills system - load, parse, and format skills for system prompts

use std::path::{Path, PathBuf};

/// Frontmatter metadata from a skill file
#[derive(Debug, Clone, Default)]
pub struct SkillFrontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
    pub disable_model_invocation: Option<bool>,
    pub extra: std::collections::HashMap<String, String>,
}

/// A loaded skill
#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub file_path: PathBuf,
    pub base_dir: PathBuf,
    pub source: SkillSource,
    pub disable_model_invocation: bool,
}

/// Where a skill was loaded from
#[derive(Debug, Clone, Copy)]
pub enum SkillSource {
    User,
    Project,
    Configured,
}

/// Result of loading skills
#[derive(Debug, Clone)]
pub struct LoadSkillsResult {
    pub skills: Vec<Skill>,
    pub diagnostics: Vec<SkillDiagnostic>,
}

/// Diagnostic message during skill loading
#[derive(Debug, Clone)]
pub struct SkillDiagnostic {
    pub path: String,
    pub message: String,
}

/// Validate a skill name
pub fn validate_name(name: &str) -> Result<(), String> {
    if name.len() > 64 {
        return Err("Skill name too long (max 64 characters)".to_string());
    }
    if name.is_empty() {
        return Err("Skill name cannot be empty".to_string());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("Skill name must be lowercase alphanumeric with hyphens only".to_string());
    }
    if name.starts_with('-') || name.ends_with('-') {
        return Err("Skill name cannot start or end with a hyphen".to_string());
    }
    if name.contains("--") {
        return Err("Skill name cannot contain double hyphens".to_string());
    }
    Ok(())
}

/// Validate a skill description
pub fn validate_description(description: &str) -> Result<(), String> {
    if description.is_empty() {
        return Err("Skill description cannot be empty".to_string());
    }
    if description.len() > 1024 {
        return Err("Skill description too long (max 1024 characters)".to_string());
    }
    Ok(())
}

/// Parse frontmatter from markdown content
pub fn parse_frontmatter(content: &str) -> (SkillFrontmatter, String) {
    let content = content.trim();
    let mut frontmatter = SkillFrontmatter::default();

    if !content.starts_with("---") {
        return (frontmatter, content.to_string());
    }

    // Find the closing ---
    let end = content[3..].find("\n---").map(|pos| pos + 3);
    let end = match end {
        Some(e) => e + 3, // include the closing ---
        None => return (frontmatter, content.to_string()),
    };

    let fm_text = &content[3..end - 3]; // content between --- and ---
    let body = content[end + 3..].trim().to_string(); // after closing ---

    // Parse key-value pairs
    for line in fm_text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(delim) = line.find(':') {
            let key = line[..delim].trim().to_lowercase();
            let value = line[delim + 1..].trim().to_string();
            match key.as_str() {
                "name" => frontmatter.name = Some(value),
                "description" => frontmatter.description = Some(value),
                "disable-model-invocation" | "disable_model_invocation" => {
                    frontmatter.disable_model_invocation = Some(value == "true");
                }
                _ => {
                    frontmatter.extra.insert(key, value);
                }
            }
        }
    }

    (frontmatter, body)
}

/// Load a skill from a file
pub fn load_skill_from_file(path: &Path, source: SkillSource) -> Option<Skill> {
    let content = std::fs::read_to_string(path).ok()?;
    let (fm, _body) = parse_frontmatter(&content);

    let name = fm.name.or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
    })?;

    let description = fm.description.unwrap_or_default();

    // Validate
    if let Err(e) = validate_name(&name) {
        tracing::warn!("Invalid skill name in {:?}: {}", path, e);
        return None;
    }
    if !description.is_empty()
        && let Err(e) = validate_description(&description)
    {
        tracing::warn!("Invalid skill description in {:?}: {}", path, e);
    }

    let base_dir = path.parent()?.to_path_buf();

    Some(Skill {
        name,
        description,
        file_path: path.to_path_buf(),
        base_dir,
        source,
        disable_model_invocation: fm.disable_model_invocation.unwrap_or(false),
    })
}

/// Discover skills from a directory
pub fn discover_skills_in_dir(dir: &Path, source: SkillSource) -> Vec<Skill> {
    let mut skills = Vec::new();
    if !dir.is_dir() {
        return skills;
    }

    // Check if this directory has a SKILL.md
    let skill_md = dir.join("SKILL.md");
    if skill_md.exists()
        && let Some(skill) = load_skill_from_file(&skill_md, source)
    {
        skills.push(skill);
        return skills;
    }

    // Load .md files directly in the root
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension()
                    && ext == "md"
                    && let Some(skill) = load_skill_from_file(&path, source)
                {
                    skills.push(skill);
                }
            } else if path.is_dir() {
                // Recurse into subdirectories to find SKILL.md files
                skills.extend(discover_skills_in_dir(&path, source));
            }
        }
    }

    skills
}

/// Load skills from multiple sources
pub fn load_skills(agent_dir: &Path, cwd: &Path, extra_paths: &[PathBuf]) -> LoadSkillsResult {
    let mut skills: Vec<Skill> = Vec::new();
    let mut diagnostics = Vec::new();
    let mut seen_names = std::collections::HashSet::new();
    let mut seen_paths = std::collections::HashSet::new();

    // Load from user skills directory: {agentDir}/skills/
    let user_skills_dir = agent_dir.join("skills");
    if user_skills_dir.is_dir() {
        for skill in discover_skills_in_dir(&user_skills_dir, SkillSource::User) {
            let canon = skill.file_path.canonicalize().unwrap_or_default();
            if seen_paths.insert(canon) {
                if seen_names.insert(skill.name.clone()) {
                    skills.push(skill);
                } else {
                    diagnostics.push(SkillDiagnostic {
                        path: skill.file_path.to_string_lossy().to_string(),
                        message: format!("Duplicate skill name '{}' (user)", skill.name),
                    });
                }
            }
        }
    }

    // Load from project skills directory: {cwd}/.pick/skills/
    let project_skills_dir = cwd.join(".pick").join("skills");
    if project_skills_dir.is_dir() {
        for skill in discover_skills_in_dir(&project_skills_dir, SkillSource::Project) {
            let canon = skill.file_path.canonicalize().unwrap_or_default();
            if seen_paths.insert(canon) {
                if seen_names.insert(skill.name.clone()) {
                    skills.push(skill);
                } else {
                    diagnostics.push(SkillDiagnostic {
                        path: skill.file_path.to_string_lossy().to_string(),
                        message: format!("Duplicate skill name '{}' (project)", skill.name),
                    });
                }
            }
        }
    }

    // Load from explicitly configured paths
    for path in extra_paths {
        if path.is_dir() {
            for skill in discover_skills_in_dir(path, SkillSource::Configured) {
                let canon = skill.file_path.canonicalize().unwrap_or_default();
                if seen_paths.insert(canon) {
                    if seen_names.insert(skill.name.clone()) {
                        skills.push(skill);
                    } else {
                        diagnostics.push(SkillDiagnostic {
                            path: skill.file_path.to_string_lossy().to_string(),
                            message: format!("Duplicate skill name '{}' (configured)", skill.name),
                        });
                    }
                }
            }
        } else if path.is_file()
            && let Some(skill) = load_skill_from_file(path, SkillSource::Configured)
        {
            let canon = skill.file_path.canonicalize().unwrap_or_default();
            if seen_paths.insert(canon) && seen_names.insert(skill.name.clone()) {
                skills.push(skill);
            }
        }
    }

    LoadSkillsResult {
        skills,
        diagnostics,
    }
}

/// Format skills as XML for the system prompt
pub fn format_skills_for_prompt(skills: &[Skill]) -> String {
    let available: Vec<&Skill> = skills
        .iter()
        .filter(|s| !s.disable_model_invocation)
        .collect();

    if available.is_empty() {
        return String::new();
    }

    let mut result = String::from(
        "The following skills provide specialized instructions for specific tasks.\n\
         Use the read tool to load a skill's file when the task matches its description.\n\
         When a skill file references a relative path, resolve it against the skill directory \
         (parent of SKILL.md / dirname of the path) and use that absolute path in tool commands.\n\n",
    );
    result.push_str("<available_skills>\n");
    for skill in &available {
        let location = skill.file_path.to_string_lossy();
        result.push_str(&format!(
            r#"  <skill>
    <name>{}</name>
    <description>{}</description>
    <location>{}</location>
  </skill>
"#,
            skill.name, skill.description, location
        ));
    }
    result.push_str("</available_skills>");
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_name() {
        assert!(validate_name("my-skill").is_ok());
        assert!(validate_name("MY-SKILL").is_err());
        assert!(validate_name("-skill").is_err());
        assert!(validate_name("skill-").is_err());
        assert!(validate_name("skill--name").is_err());
        assert!(validate_name("").is_err());
        assert!(validate_name("a".repeat(65).as_str()).is_err());
    }

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
name: test-skill
description: A test skill
disable-model-invocation: true
---

# Skill Content

Hello world"#;

        let (fm, body) = parse_frontmatter(content);
        assert_eq!(fm.name.as_deref(), Some("test-skill"));
        assert_eq!(fm.description.as_deref(), Some("A test skill"));
        assert_eq!(fm.disable_model_invocation, Some(true));
        assert!(body.contains("Hello world"));
    }

    #[test]
    fn test_parse_frontmatter_no_fm() {
        let content = "# Just a normal markdown file\n\nSome content";
        let (fm, body) = parse_frontmatter(content);
        assert!(fm.name.is_none());
        assert_eq!(body, content);
    }

    #[test]
    fn test_format_skills_for_prompt() {
        let skills = vec![
            Skill {
                name: "test-1".to_string(),
                description: "First skill".to_string(),
                file_path: PathBuf::from("/tmp/test-1.md"),
                base_dir: PathBuf::from("/tmp"),
                source: SkillSource::User,
                disable_model_invocation: false,
            },
            Skill {
                name: "test-2".to_string(),
                description: "Second skill".to_string(),
                file_path: PathBuf::from("/tmp/test-2.md"),
                base_dir: PathBuf::from("/tmp"),
                source: SkillSource::User,
                disable_model_invocation: true,
            },
        ];

        let formatted = format_skills_for_prompt(&skills);
        // Check intro text
        assert!(formatted.contains("read tool to load a skill's file"));
        assert!(formatted.contains("skill directory"));
        // Check test-1 with all new fields
        assert!(formatted.contains("test-1"));
        assert!(formatted.contains("First skill"));
        assert!(formatted.contains("/tmp/test-1.md"));
        assert!(formatted.contains("<location>"));
        // test-2 should be excluded (disabled for model invocation)
        assert!(!formatted.contains("test-2"));
    }
}
