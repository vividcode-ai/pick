use super::{Action, Ruleset};

pub fn wildcard_match(pattern: &str, input: &str) -> bool {
    let pattern_lower = pattern.to_lowercase();
    let input_lower = input.to_lowercase();

    if pattern_lower == "*" {
        return true;
    }

    if !pattern_lower.contains('*') {
        return pattern_lower == input_lower;
    }

    let parts: Vec<&str> = pattern_lower.split('*').collect();
    let mut remaining = input_lower.as_str();

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !remaining.starts_with(part) {
                return false;
            }
            remaining = &remaining[part.len()..];
        } else if i == parts.len() - 1 {
            if !remaining.ends_with(part) {
                return false;
            }
        } else {
            match remaining.find(part) {
                Some(pos) => {
                    remaining = &remaining[pos + part.len()..];
                }
                None => return false,
            }
        }
    }

    true
}

pub fn evaluate(
    permission: &str,
    pattern: &str,
    rulesets: &[&Ruleset],
) -> Action {
    let mut result = Action::Ask;

    for ruleset in rulesets {
        for rule in ruleset.rules() {
            if wildcard_match(&rule.permission, permission)
                && wildcard_match(&rule.pattern, pattern)
            {
                result = rule.action;
            }
        }
    }

    result
}

pub fn evaluate_tool(
    tool_name: &str,
    tool_args: &str,
    rulesets: &[&Ruleset],
) -> Action {
    let permission_key = super::tool_to_permission_key(tool_name);

    let pattern = if permission_key == "bash" {
        bash_command_prefix(tool_args)
    } else {
        String::from("*")
    };

    evaluate(permission_key, &pattern, rulesets)
}

pub fn evaluate_tool_allow(
    tool_name: &str,
    tool_args: &str,
    rulesets: &[&Ruleset],
) -> bool {
    let action = evaluate_tool(tool_name, tool_args, rulesets);
    action == Action::Allow
}

pub fn is_tool_disabled(tool_name: &str, rulesets: &[&Ruleset]) -> bool {
    let perm_key = super::tool_to_permission_key(tool_name);

    // Find the LAST rule where pattern is exactly "*" (catch-all) matching this permission.
    // This is separate from evaluate() which uses wildcard_match(pattern, "*") — a specific
    // rule like ".pick/plans/*.md allow" would match "*" via wildcard but its pattern is
    // NOT exactly "*", so it won't be considered a catch-all.
    let last_catch_all = rulesets.iter().flat_map(|rs| rs.rules()).rev().find(|r| {
        wildcard_match(&r.permission, perm_key) && r.pattern == "*"
    });

    match last_catch_all {
        Some(rule) => rule.action == Action::Deny,
        None => false, // no catch-all rule, tool is not disabled
    }
}

pub fn bash_command_prefix(args: &str) -> String {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() {
        return String::new();
    }

    let first = parts[0].to_lowercase();

    if (first == "git" || first == "npm" || first == "cargo" || first == "docker"
        || first == "kubectl" || first == "gh" || first == "npx" || first == "bun"
        || first == "yarn" || first == "dotnet" || first == "make"
        || first == "pip" || first == "rustup" || first == "go"
        || first == "apt" || first == "apt-get" || first == "brew"
        || first == "pacman" || first == "snap")
        && parts.len() > 1
    {
        format!("{} {}", first, parts[1].to_lowercase())
    } else {
        first
    }
}

pub fn extract_tool_args(tc: &pick_ai::types::ToolCall) -> String {
    if let Some(cmd) = tc.arguments.get("command").and_then(|c| c.as_str()) {
        cmd.to_string()
    } else if let Some(path) = tc.arguments.get("path").and_then(|p| p.as_str()) {
        path.to_string()
    } else {
        tc.arguments.to_string()
    }
}

pub fn check_permission(
    tool_name: &str,
    tool_args: &str,
    rulesets: &[&Ruleset],
) -> Result<(), String> {
    match evaluate_tool(tool_name, tool_args, rulesets) {
        Action::Allow => Ok(()),
        Action::Deny => Err(format!(
            "Permission denied: '{}' is not allowed in the current mode",
            tool_name
        )),
        Action::Ask => Err(format!(
            "Permission required: '{}' needs user approval in the current mode",
            tool_name
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::{Rule, tool_to_permission_key as ttpk};

    #[test]
    fn test_wildcard_match_exact() {
        assert!(wildcard_match("edit", "edit"));
        assert!(!wildcard_match("edit", "read"));
    }

    #[test]
    fn test_wildcard_match_star() {
        assert!(wildcard_match("*", "anything"));
    }

    #[test]
    fn test_wildcard_match_prefix() {
        assert!(wildcard_match("git *", "git push"));
        assert!(wildcard_match("git *", "git commit -m foo"));
        assert!(!wildcard_match("git *", "githubb push"));
    }

    #[test]
    fn test_wildcard_match_suffix() {
        assert!(wildcard_match("*.md", "README.md"));
        assert!(!wildcard_match("*.md", "README.txt"));
    }

    #[test]
    fn test_wildcard_match_glob() {
        assert!(wildcard_match(".opencode/plans/*.md", ".opencode/plans/plan-123.md"));
        assert!(!wildcard_match(".opencode/plans/*.md", ".opencode/other/file.txt"));
    }

    #[test]
    fn test_evaluate_last_match_wins() {
        let deny_all = Ruleset::new(vec![
            Rule::new("edit", "*", Action::Deny),
        ]);
        let allow_md = Ruleset::new(vec![
            Rule::new("edit", "*.md", Action::Allow),
        ]);

        // deny all first, then allow *.md — last match wins
        assert_eq!(evaluate("edit", "foo.md", &[&deny_all, &allow_md]), Action::Allow);
        assert_eq!(evaluate("edit", "foo.rs", &[&deny_all, &allow_md]), Action::Deny);
    }

    #[test]
    fn test_evaluate_default_ask() {
        let empty = Ruleset::new(vec![]);
        assert_eq!(evaluate("read", "*", &[&empty]), Action::Ask);
    }

    #[test]
    fn test_bash_command_prefix_simple() {
        assert_eq!(bash_command_prefix("ls -la"), "ls");
        assert_eq!(bash_command_prefix("cat file.txt"), "cat");
    }

    #[test]
    fn test_bash_command_prefix_git() {
        assert_eq!(bash_command_prefix("git push origin main"), "git push");
        assert_eq!(bash_command_prefix("git commit -m 'fix'"), "git commit");
        assert_eq!(bash_command_prefix("git log --oneline"), "git log");
        assert_eq!(bash_command_prefix("git diff HEAD"), "git diff");
    }

    #[test]
    fn test_bash_command_prefix_npm() {
        assert_eq!(bash_command_prefix("npm install"), "npm install");
        assert_eq!(bash_command_prefix("npm run build"), "npm run");
    }

    #[test]
    fn test_evaluate_tool_bash() {
        let plan_rules = Ruleset::new(vec![
            Rule::new("bash", "ls", Action::Allow),
            Rule::new("bash", "cat", Action::Allow),
            Rule::new("bash", "rg", Action::Allow),
            Rule::new("bash", "*", Action::Deny),
        ]);

        // *: Deny at the end overrides specific allows with last-match-wins
        assert_eq!(evaluate_tool("bash", "ls -la", &[&plan_rules]), Action::Deny);
        // No specific rule for sed, catch-all deny applies
        assert_eq!(evaluate_tool("bash", "sed -i 's/foo/bar/' file.txt", &[&plan_rules]), Action::Deny);
    }

    #[test]
    fn test_evaluate_tool_bash_no_catchall() {
        // With only specific allows (no *: deny), unmatched commands return Ask
        let plan_rules = Ruleset::new(vec![
            Rule::new("bash", "ls", Action::Allow),
            Rule::new("bash", "cat", Action::Allow),
            Rule::new("bash", "rg", Action::Allow),
        ]);

        assert_eq!(evaluate_tool("bash", "ls -la", &[&plan_rules]), Action::Allow);
        assert_eq!(evaluate_tool("bash", "cat file.txt", &[&plan_rules]), Action::Allow);
        assert_eq!(evaluate_tool("bash", "sed -i 's/foo/bar/' file.txt", &[&plan_rules]), Action::Ask);
    }

    #[test]
    fn test_is_tool_disabled() {
        let edit_deny = Ruleset::new(vec![
            Rule::new("edit", "*", Action::Deny),
        ]);
        assert!(is_tool_disabled("write", &[&edit_deny]));
        assert!(is_tool_disabled("edit", &[&edit_deny]));

        let bash_allowed = Ruleset::new(vec![
            Rule::new("bash", "ls", Action::Allow),
            Rule::new("bash", "*", Action::Deny),
        ]);
        // Has catch-all *: deny, so tool IS disabled
        // (specific allows like "ls" don't make the tool visible; they apply at runtime)
        assert!(is_tool_disabled("bash", &[&bash_allowed]));

        let bash_deny = Ruleset::new(vec![
            Rule::new("bash", "*", Action::Deny),
        ]);
        assert!(is_tool_disabled("bash", &[&bash_deny]));
    }

    #[test]
    fn test_is_tool_disabled_with_specific_allow() {
        // Plan mode ruleset: edit denied globally, except plan files
        let plan_rules = Ruleset::new(vec![
            Rule::new("edit", "*", Action::Deny),
            Rule::new("edit", ".pick/plans/*.md", Action::Allow),
        ]);
        // write/edit tools should be DISABLED (hidden from LLM) in plan mode
        assert!(is_tool_disabled("write", &[&plan_rules]));
        assert!(is_tool_disabled("edit", &[&plan_rules]));
        assert!(is_tool_disabled("apply_patch", &[&plan_rules]));
        // read is not affected
        assert!(!is_tool_disabled("read", &[&plan_rules]));
    }

    #[test]
    fn test_evaluate_tool_edit() {
        let plan_rules = Ruleset::new(vec![
            Rule::new("edit", "*", Action::Deny),
            Rule::new("edit", ".pick/plans/*.md", Action::Allow),
        ]);

        assert_eq!(evaluate_tool("write", "anything", &[&plan_rules]), Action::Deny);
        assert_eq!(evaluate_tool("edit", "anything", &[&plan_rules]), Action::Deny);
        assert_eq!(evaluate_tool("read", "", &[&plan_rules]), Action::Ask); // no read rule
    }

    #[test]
    fn test_check_permission_denied() {
        let rules = Ruleset::new(vec![
            Rule::new("edit", "*", Action::Deny),
        ]);
        assert!(check_permission("write", "file.rs", &[&rules]).is_err());
        // read has no rules, defaults to Ask
        // read has no rules, defaults to Ask which is treated as error by check_permission
        assert!(check_permission("read", "file.rs", &[&rules]).is_err());
        // check_permission with Allow rules succeeds
        let allow_rules = Ruleset::new(vec![
            Rule::new("read", "*", Action::Allow),
        ]);
        assert!(check_permission("read", "file.rs", &[&allow_rules]).is_ok());
    }

    #[test]
    fn test_is_tool_disabled_edit() {
        let rules = Ruleset::new(vec![
            Rule::new("edit", "*", Action::Deny),
        ]);
        assert!(is_tool_disabled("write", &[&rules]));
        assert!(is_tool_disabled("edit", &[&rules]));
        assert!(is_tool_disabled("apply_patch", &[&rules]));
        assert!(!is_tool_disabled("read", &[&rules]));
    }

    #[test]
    fn test_tool_to_permission_key() {
        assert_eq!(ttpk("write"), "edit");
        assert_eq!(ttpk("edit"), "edit");
        assert_eq!(ttpk("apply_patch"), "edit");
        assert_eq!(ttpk("find"), "glob");
        assert_eq!(ttpk("read"), "read");
        assert_eq!(ttpk("bash"), "bash");
    }
}

