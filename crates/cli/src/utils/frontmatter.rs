//! Frontmatter parsing

/// Parsed frontmatter result
pub struct ParsedFrontmatter<T> {
    pub frontmatter: T,
    pub body: String,
}

/// Extract YAML frontmatter string and body from content
fn extract_frontmatter(content: &str) -> (Option<String>, String) {
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");

    if !normalized.starts_with("---") {
        return (None, normalized);
    }

    if let Some(end_index) = normalized[3..].find("\n---") {
        let yaml_string = normalized[4..3 + end_index].to_string();
        let body = normalized[3 + end_index + 4..].trim().to_string();
        (Some(yaml_string), body)
    } else {
        (None, normalized)
    }
}

/// Parse frontmatter from content
pub fn parse_frontmatter(
    content: &str,
) -> ParsedFrontmatter<std::collections::HashMap<String, String>> {
    let (yaml_string, body) = extract_frontmatter(content);
    let frontmatter = match yaml_string {
        Some(yaml) => parse_simple_yaml(&yaml),
        None => std::collections::HashMap::new(),
    };
    ParsedFrontmatter { frontmatter, body }
}

/// Strip frontmatter from content, returning just the body
pub fn strip_frontmatter(content: &str) -> String {
    parse_frontmatter(content).body
}

/// Expand a /skill:name command into a <skill> XML block for the agent.
/// Returns None if the text is not a skill command or the skill is not found.
pub fn expand_skill_command(text: &str, skills: &[pick_agent::skills::Skill]) -> Option<String> {
    if !text.starts_with("/skill:") {
        return None;
    }
    let rest = &text[7..];
    let space_idx = rest.find(' ');
    let skill_name = match space_idx {
        Some(i) => rest[..i].trim(),
        None => rest.trim(),
    };
    let args = match space_idx {
        Some(i) => rest[i + 1..].trim(),
        None => "",
    };
    let skill = skills.iter().find(|s| s.name == skill_name)?;
    let content = std::fs::read_to_string(&skill.file_path).ok()?;
    let body = strip_frontmatter(&content).trim().to_string();
    let base_dir = skill.base_dir.to_string_lossy();
    let expanded = if args.is_empty() {
        format!(
            r#"<skill name="{}" location="{}">
References are relative to {}.

{}
</skill>"#,
            skill.name,
            skill.file_path.display(),
            base_dir,
            body
        )
    } else {
        format!(
            r#"<skill name="{}" location="{}">
References are relative to {}.

{}
</skill>

{}"#,
            skill.name,
            skill.file_path.display(),
            base_dir,
            body,
            args
        )
    };
    Some(expanded)
}

/// Simple YAML key-value parser (no nested structures)
fn parse_simple_yaml(yaml: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for line in yaml.lines() {
        if let Some(idx) = line.find(':') {
            let key = line[..idx].trim().to_string();
            let value = line[idx + 1..].trim().trim_matches('"').to_string();
            if !key.is_empty() {
                map.insert(key, value);
            }
        }
    }
    map
}
