//! Prompt template loading, parsing, and expansion

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::CONFIG_DIR_NAME;
use crate::core::source_info::{
    SourceInfo, SourceScope, SyntheticSourceOptions, create_synthetic_source_info,
};

/// A loaded prompt template from a markdown file
#[derive(Debug, Clone)]
pub struct PromptTemplate {
    pub name: String,
    pub description: String,
    pub argument_hint: Option<String>,
    pub content: String,
    pub source_info: SourceInfo,
    pub file_path: String,
}

/// Parse command arguments respecting quoted strings (bash-style)
pub fn parse_command_args(args_string: &str) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;

    for c in args_string.chars() {
        if let Some(quote) = in_quote {
            if c == quote {
                in_quote = None;
            } else {
                current.push(c);
            }
        } else if c == '"' || c == '\'' {
            in_quote = Some(c);
        } else if c.is_whitespace() {
            if !current.is_empty() {
                args.push(current.clone());
                current.clear();
            }
        } else {
            current.push(c);
        }
    }

    if !current.is_empty() {
        args.push(current);
    }

    args
}

/// Substitute argument placeholders in template content
/// Supports: $1, $2, $@, $ARGUMENTS, ${@:N}, ${@:N:L}
pub fn substitute_args(content: &str, args: &[String]) -> String {
    let mut result = content.to_string();

    // Replace $1, $2, etc. with positional args FIRST
    let re = regex::Regex::new(r"\$(\d+)").unwrap();
    result = re
        .replace_all(&result, |caps: &regex::Captures| {
            let num: usize = caps.get(1).unwrap().as_str().parse().unwrap_or(0);
            if num > 0 {
                args.get(num - 1).map(|s| s.as_str()).unwrap_or("")
            } else {
                ""
            }
            .to_string()
        })
        .to_string();

    // Replace ${@:N} or ${@:N:L} with sliced args
    let re_slice = regex::Regex::new(r"\$\{@:(\d+)(?::(\d+))?\}").unwrap();
    result = re_slice
        .replace_all(&result, |caps: &regex::Captures| {
            let start_raw: usize = caps.get(1).unwrap().as_str().parse().unwrap_or(1);
            let start = if start_raw == 0 { 0 } else { start_raw - 1 };

            if let Some(len_match) = caps.get(2) {
                let length: usize = len_match.as_str().parse().unwrap_or(0);
                args.iter()
                    .skip(start)
                    .take(length)
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                args.iter()
                    .skip(start)
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        })
        .to_string();

    // $ARGUMENTS
    let all_args = args.join(" ");
    result = result.replace("$ARGUMENTS", &all_args);

    // $@
    result = result.replace("$@", &all_args);

    result
}

/// Simple frontmatter parser for key-value pairs (description, argument-hint)
pub(crate) fn parse_frontmatter(content: &str) -> (HashMap<String, String>, String) {
    let content = content.replace("\r\n", "\n").replace('\r', "\n");

    if !content.starts_with("---") {
        return (HashMap::new(), content);
    }

    if let Some(end) = content[3..].find("\n---") {
        let yaml_section = &content[4..3 + end];
        let body = content[3 + end + 4..].trim().to_string();

        let mut frontmatter = HashMap::new();
        for line in yaml_section.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(colon_pos) = line.find(':') {
                let key = line[..colon_pos].trim().to_string();
                let value = line[colon_pos + 1..].trim().to_string();
                frontmatter.insert(key, value);
            }
        }

        (frontmatter, body)
    } else {
        (HashMap::new(), content)
    }
}

fn load_template_from_file(file_path: &Path, source_info: SourceInfo) -> Option<PromptTemplate> {
    let content = std::fs::read_to_string(file_path).ok()?;

    let (frontmatter, body) = parse_frontmatter(&content);

    let name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unnamed".to_string());

    let description = frontmatter
        .get("description")
        .cloned()
        .or_else(|| {
            body.lines()
                .find(|l| !l.trim().is_empty())
                .map(|first_line| {
                    let trimmed = first_line.trim();
                    if trimmed.len() > 60 {
                        format!("{}...", &trimmed[..60])
                    } else {
                        trimmed.to_string()
                    }
                })
        })
        .unwrap_or_default();

    let argument_hint = frontmatter.get("argument-hint").cloned();

    Some(PromptTemplate {
        name,
        description,
        argument_hint,
        content: body.to_string(),
        source_info,
        file_path: file_path.to_string_lossy().to_string(),
    })
}

fn load_templates_from_dir(
    dir: &Path,
    get_source_info: &dyn Fn(&str) -> SourceInfo,
) -> Vec<PromptTemplate> {
    let mut templates = Vec::new();

    if !dir.exists() {
        return templates;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return templates,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Follow symlinks
        let metadata = match path.symlink_metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let is_file = if metadata.is_symlink() {
            std::fs::metadata(&path)
                .map(|m| m.is_file())
                .unwrap_or(false)
        } else {
            metadata.is_file()
        };

        if is_file {
            if let Some(ext) = path.extension() {
                if ext == "md" {
                    let file_path_str = path.to_string_lossy().to_string();
                    let source_info = get_source_info(&file_path_str);
                    if let Some(template) = load_template_from_file(&path, source_info) {
                        templates.push(template);
                    }
                }
            }
        }
    }

    templates
}

/// Options for loading prompt templates
pub struct LoadPromptTemplatesOptions {
    pub cwd: PathBuf,
    pub agent_dir: PathBuf,
    pub prompt_paths: Vec<PathBuf>,
    pub include_defaults: bool,
}

/// Load all prompt templates from global, project, and explicit paths
pub fn load_prompt_templates(options: LoadPromptTemplatesOptions) -> Vec<PromptTemplate> {
    let resolved_cwd = options.cwd;
    let resolved_agent_dir = options.agent_dir;
    let prompt_paths = options.prompt_paths;
    let include_defaults = options.include_defaults;

    let mut templates: Vec<PromptTemplate> = Vec::new();

    let global_prompts_dir = resolved_agent_dir.join("prompts");
    let project_prompts_dir = resolved_cwd.join(CONFIG_DIR_NAME).join("prompts");

    let get_source_info = |resolved_path: &str| -> SourceInfo {
        let resolved = Path::new(resolved_path);

        if is_under_path(resolved, &global_prompts_dir) {
            create_synthetic_source_info(
                resolved_path,
                SyntheticSourceOptions {
                    source: "local".to_string(),
                    scope: Some(SourceScope::User),
                    origin: None,
                    base_dir: Some(global_prompts_dir.to_string_lossy().to_string()),
                },
            )
        } else if is_under_path(resolved, &project_prompts_dir) {
            create_synthetic_source_info(
                resolved_path,
                SyntheticSourceOptions {
                    source: "local".to_string(),
                    scope: Some(SourceScope::Project),
                    origin: None,
                    base_dir: Some(project_prompts_dir.to_string_lossy().to_string()),
                },
            )
        } else {
            let base_dir = if Path::new(resolved_path).is_dir() {
                resolved_path.to_string()
            } else {
                Path::new(resolved_path)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default()
            };
            create_synthetic_source_info(
                resolved_path,
                SyntheticSourceOptions {
                    source: "local".to_string(),
                    scope: None,
                    origin: None,
                    base_dir: Some(base_dir),
                },
            )
        }
    };

    if include_defaults {
        templates.extend(load_templates_from_dir(
            &global_prompts_dir,
            &get_source_info,
        ));
        templates.extend(load_templates_from_dir(
            &project_prompts_dir,
            &get_source_info,
        ));
    }

    // Load explicit prompt paths
    for raw_path in &prompt_paths {
        let resolved_path = if raw_path.is_absolute() {
            raw_path.clone()
        } else {
            resolved_cwd.join(raw_path)
        };

        if !resolved_path.exists() {
            continue;
        }

        if resolved_path.is_dir() {
            templates.extend(load_templates_from_dir(&resolved_path, &get_source_info));
        } else if resolved_path.is_file() {
            if let Some(ext) = resolved_path.extension() {
                if ext == "md" {
                    let path_str = resolved_path.to_string_lossy().to_string();
                    let source_info = get_source_info(&path_str);
                    if let Some(template) = load_template_from_file(&resolved_path, source_info) {
                        templates.push(template);
                    }
                }
            }
        }
    }

    templates
}

fn is_under_path(target: &Path, root: &Path) -> bool {
    let canonical_root = if root.is_absolute() {
        root.to_path_buf()
    } else {
        PathBuf::new()
    };

    let target_str = target.to_string_lossy().to_lowercase();
    let root_str = canonical_root.to_string_lossy().to_lowercase();

    target_str == root_str
        || target_str.starts_with(&format!("{}\\", root_str))
        || target_str.starts_with(&format!("{}/", root_str))
}

/// Expand a prompt template if the text matches a template name.
/// Returns the expanded content or the original text if not a template.
pub fn expand_prompt_template(text: &str, templates: &[PromptTemplate]) -> String {
    if !text.starts_with('/') {
        return text.to_string();
    }

    let re = regex::Regex::new(r"^/(\S+)(?:\s+([\s\S]*))?$").unwrap();
    if let Some(caps) = re.captures(text) {
        let template_name = caps.get(1).unwrap().as_str();
        let args_string = caps.get(2).map(|m| m.as_str()).unwrap_or("");

        if let Some(template) = templates.iter().find(|t| t.name == template_name) {
            let args = parse_command_args(args_string);
            return substitute_args(&template.content, &args);
        }
    }

    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command_args_simple() {
        assert_eq!(parse_command_args("hello world"), vec!["hello", "world"]);
    }

    #[test]
    fn test_parse_command_args_quoted() {
        assert_eq!(
            parse_command_args("hello \"foo bar\" world"),
            vec!["hello", "foo bar", "world"]
        );
    }

    #[test]
    fn test_parse_command_args_single_quoted() {
        assert_eq!(
            parse_command_args("hello 'foo bar'"),
            vec!["hello", "foo bar"]
        );
    }

    #[test]
    fn test_substitute_args_positional() {
        let args = vec!["hello".to_string(), "world".to_string()];
        assert_eq!(substitute_args("$1 $2", &args), "hello world");
    }

    #[test]
    fn test_substitute_args_all() {
        let args = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(substitute_args("$@", &args), "a b c");
    }

    #[test]
    fn test_substitute_args_arguments() {
        let args = vec!["a".to_string(), "b".to_string()];
        assert_eq!(substitute_args("$ARGUMENTS", &args), "a b");
    }

    #[test]
    fn test_substitute_args_slice() {
        let args = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(substitute_args("${@:2}", &args), "b c");
        assert_eq!(substitute_args("${@:2:1}", &args), "b");
    }

    #[test]
    fn test_parse_frontmatter_basic() {
        let content =
            "---\ndescription: A test template\nargument-hint: <name>\n---\n\nHello, {{name}}!";
        let (fm, _body) = parse_frontmatter(content);
        assert_eq!(fm.get("description").unwrap(), "A test template");
        assert_eq!(fm.get("argument-hint").unwrap(), "<name>");
    }

    #[test]
    fn test_expand_prompt_template() {
        let template = PromptTemplate {
            name: "test".to_string(),
            description: "A test".to_string(),
            argument_hint: Some("<input>".to_string()),
            content: "You said: $1".to_string(),
            source_info: create_synthetic_source_info(
                "/tmp/test.md",
                SyntheticSourceOptions {
                    source: "local".to_string(),
                    scope: None,
                    origin: None,
                    base_dir: None,
                },
            ),
            file_path: "/tmp/test.md".to_string(),
        };

        let result = expand_prompt_template("/test hello", &[template]);
        assert_eq!(result, "You said: hello");
    }

    #[test]
    fn test_no_expansion_without_slash() {
        let templates = vec![];
        let result = expand_prompt_template("just regular text", &templates);
        assert_eq!(result, "just regular text");
    }
}
