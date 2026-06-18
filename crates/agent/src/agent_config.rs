//! Agent configuration types and discovery

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Source of an agent definition
#[derive(Debug, Clone, PartialEq)]
pub enum AgentSource {
    User,
    Project,
}

/// Scope for agent discovery
#[derive(Debug, Clone, PartialEq)]
pub enum AgentScope {
    User,
    Project,
    Both,
}

/// A discovered agent definition
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub name: String,
    pub description: String,
    pub tools: Option<Vec<String>>,
    pub model: Option<String>,
    pub system_prompt: String,
    pub source: AgentSource,
    pub file_path: PathBuf,
}

/// Result from discover_agents
#[derive(Debug, Clone)]
pub struct AgentDiscoveryResult {
    pub agents: Vec<AgentConfig>,
    pub project_agents_dir: Option<PathBuf>,
}

/// Load agent definitions from a directory (scans *.md files)
pub fn load_agents_from_dir(dir: &Path, source: AgentSource) -> Vec<AgentConfig> {
    let mut agents = Vec::new();

    if !dir.exists() || !dir.is_dir() {
        return agents;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return agents,
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let file_path = entry.path();
        if file_path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        if !file_path.is_file() && !file_path.is_symlink() {
            continue;
        }

        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let (frontmatter, body) = parse_agent_frontmatter(&content);

        let name = match frontmatter.get("name") {
            Some(n) if !n.is_empty() => n.clone(),
            _ => continue,
        };

        let description = match frontmatter.get("description") {
            Some(d) if !d.is_empty() => d.clone(),
            _ => continue,
        };

        let tools = frontmatter.get("tools").and_then(|t| {
            let list: Vec<String> = t
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if list.is_empty() { None } else { Some(list) }
        });

        let model = frontmatter.get("model").filter(|m| !m.is_empty()).cloned();

        agents.push(AgentConfig {
            name,
            description,
            tools,
            model,
            system_prompt: body,
            source: source.clone(),
            file_path,
        });
    }

    agents
}

/// Find the nearest project agents directory by walking up from cwd
pub fn find_nearest_project_agents_dir(cwd: &Path) -> Option<PathBuf> {
    let mut current = Some(cwd.to_path_buf());

    while let Some(dir) = current {
        let candidate = dir.join(".pick").join("agents");
        if candidate.is_dir() {
            return Some(candidate);
        }
        current = dir.parent().map(|p| p.to_path_buf());
    }

    None
}

/// Discover agents from user and/or project directories
pub fn discover_agents(cwd: &Path, agent_dir: &Path, scope: &AgentScope) -> AgentDiscoveryResult {
    let user_dir = agent_dir.join("agents");
    let project_agents_dir = find_nearest_project_agents_dir(cwd);

    let user_agents = if *scope == AgentScope::Project {
        Vec::new()
    } else {
        load_agents_from_dir(&user_dir, AgentSource::User)
    };

    let project_agents = if *scope == AgentScope::User || project_agents_dir.is_none() {
        Vec::new()
    } else {
        load_agents_from_dir(project_agents_dir.as_ref().unwrap(), AgentSource::Project)
    };

    let mut agent_map: HashMap<String, AgentConfig> = HashMap::new();

    match scope {
        AgentScope::Both | AgentScope::User => {
            for agent in user_agents {
                agent_map.insert(agent.name.clone(), agent);
            }
        }
        AgentScope::Project => {}
    }

    match scope {
        AgentScope::Both | AgentScope::Project => {
            for agent in project_agents {
                agent_map.insert(agent.name.clone(), agent);
            }
        }
        AgentScope::User => {}
    }

    let agents: Vec<AgentConfig> = agent_map.into_values().collect();

    AgentDiscoveryResult {
        agents,
        project_agents_dir,
    }
}

/// Parse YAML frontmatter from markdown content, returning (fields, body)
fn parse_agent_frontmatter(content: &str) -> (HashMap<String, String>, String) {
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");

    if !normalized.starts_with("---") {
        return (HashMap::new(), normalized);
    }

    if let Some(end_index) = normalized[3..].find("\n---") {
        let yaml_string = &normalized[4..3 + end_index];
        let body = normalized[3 + end_index + 4..].trim().to_string();
        let frontmatter = parse_simple_yaml(yaml_string);
        (frontmatter, body)
    } else {
        (HashMap::new(), normalized)
    }
}

/// Simple YAML key-value parser (no nested structures)
fn parse_simple_yaml(yaml: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
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

/// Format agent list for display
pub fn format_agent_list(agents: &[AgentConfig], max_items: usize) -> (String, usize) {
    if agents.is_empty() {
        return ("none".to_string(), 0);
    }
    let listed = agents.iter().take(max_items);
    let remaining = agents.len().saturating_sub(max_items);
    let text = listed
        .map(|a| {
            let source = match a.source {
                AgentSource::User => "user",
                AgentSource::Project => "project",
            };
            format!("{} ({}): {}", a.name, source, a.description)
        })
        .collect::<Vec<_>>()
        .join("; ");
    (text, remaining)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_agent_frontmatter_full() {
        let content = r#"---
name: scout
description: Fast codebase recon
tools: read, grep, find, ls, bash
model: claude-haiku-4-5
---

You are a scout. Investigate codebases."#;

        let (fm, body) = parse_agent_frontmatter(content);
        assert_eq!(fm.get("name").unwrap(), "scout");
        assert_eq!(fm.get("description").unwrap(), "Fast codebase recon");
        assert_eq!(fm.get("tools").unwrap(), "read, grep, find, ls, bash");
        assert_eq!(fm.get("model").unwrap(), "claude-haiku-4-5");
        assert!(body.contains("You are a scout"));
    }

    #[test]
    fn test_parse_agent_frontmatter_minimal() {
        let content = r#"---
name: worker
description: General-purpose agent
---

Do whatever is needed."#;

        let (fm, body) = parse_agent_frontmatter(content);
        assert_eq!(fm.get("name").unwrap(), "worker");
        assert_eq!(fm.get("description").unwrap(), "General-purpose agent");
        assert!(fm.get("tools").is_none());
        assert!(fm.get("model").is_none());
        assert!(body.contains("Do whatever is needed"));
    }

    #[test]
    fn test_parse_agent_frontmatter_missing_name() {
        let content = r#"---
description: No name here
---

Some body."#;

        let (fm, _) = parse_agent_frontmatter(content);
        assert!(fm.get("name").is_none());
    }

    #[test]
    fn test_parse_agent_frontmatter_no_frontmatter() {
        let content = "Just a plain markdown file without frontmatter.";
        let (fm, body) = parse_agent_frontmatter(content);
        assert!(fm.is_empty());
        assert_eq!(body, content);
    }

    #[test]
    fn test_load_agents_from_empty_dir() {
        let dir = std::env::temp_dir().join("test_agents_empty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let agents = load_agents_from_dir(&dir, AgentSource::User);
        assert!(agents.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_agents_from_dir() {
        let dir = std::env::temp_dir().join("test_agents_load");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let scout_content = r#"---
name: scout
description: Codebase recon
tools: read, grep, find, ls
model: claude-haiku-4-5
---

Scout system prompt."#;

        let worker_content = r#"---
name: worker
description: General purpose
---

Worker system prompt."#;

        std::fs::write(dir.join("scout.md"), scout_content).unwrap();
        std::fs::write(dir.join("worker.md"), worker_content).unwrap();

        let agents = load_agents_from_dir(&dir, AgentSource::User);
        assert_eq!(agents.len(), 2);

        let scout = agents.iter().find(|a| a.name == "scout").unwrap();
        assert_eq!(scout.description, "Codebase recon");
        assert_eq!(scout.tools.as_ref().unwrap().len(), 4);
        assert_eq!(scout.model.as_ref().unwrap(), "claude-haiku-4-5");
        assert!(scout.system_prompt.contains("Scout system prompt"));
        assert_eq!(scout.source, AgentSource::User);

        let worker = agents.iter().find(|a| a.name == "worker").unwrap();
        assert!(worker.tools.is_none());
        assert!(worker.model.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_format_agent_list() {
        let agents = vec![
            AgentConfig {
                name: "scout".to_string(),
                description: "Recon".to_string(),
                tools: None,
                model: None,
                system_prompt: "".to_string(),
                source: AgentSource::User,
                file_path: PathBuf::new(),
            },
            AgentConfig {
                name: "planner".to_string(),
                description: "Planning".to_string(),
                tools: None,
                model: None,
                system_prompt: "".to_string(),
                source: AgentSource::Project,
                file_path: PathBuf::new(),
            },
        ];

        let (text, remaining) = format_agent_list(&agents, 1);
        assert!(text.contains("scout"));
        assert!(text.contains("user"));
        assert_eq!(remaining, 1);
    }

    #[test]
    fn test_discover_agents_no_project_dir() {
        let cwd = std::env::temp_dir().join("test_discover_no_project");
        let agent_dir = cwd.join(".pick").join("agent");
        std::fs::create_dir_all(&agent_dir).unwrap();

        let result = discover_agents(&cwd, &agent_dir.join(".."), &AgentScope::User);
        assert!(result.agents.is_empty());
        assert!(result.project_agents_dir.is_none());
    }
}
