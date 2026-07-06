use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExecDecision {
    Allow,
    Prompt,
    Forbidden,
}

#[derive(Debug, Clone)]
pub struct ExecRule {
    pub pattern: String,
    pub decision: ExecDecision,
    pub justification: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExecPolicy {
    rules: Vec<ExecRule>,
    known_safe: HashSet<String>,
    known_dangerous: HashSet<String>,
}

impl Default for ExecPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecPolicy {
    pub fn new() -> Self {
        let known_safe: HashSet<String> = [
            "ls", "cat", "head", "tail", "rg", "grep", "find", "which", "stat", "wc", "diff",
            "sort", "uniq", "echo", "pwd", "type", "where", "dir", "more", "less", "date", "cal",
            "df", "du", "uptime",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let known_dangerous: HashSet<String> = [
            "rm",
            "mv",
            "sudo",
            "chmod",
            "chown",
            "mkfs",
            "dd",
            "shutdown",
            "reboot",
            "kill",
            "pkill",
            "iptables",
            "curl",
            "wget",
            "nc",
            "ncat",
            "ssh",
            "scp",
            "sftp",
            "rsync",
            "python",
            "python3",
            "pip",
            "pip3",
            "perl",
            "ruby",
            "php",
            "node",
            "deno",
            "mount",
            "umount",
            "passwd",
            "chsh",
            "useradd",
            "usermod",
            "crontab",
            "systemctl",
            "service",
            "docker",
            "podman",
            "kubectl",
            "helm",
            "go",
            "rustc",
            "gcc",
            "clang",
            "make",
            "cmake",
            "tar",
            "gzip",
            "zip",
            "unzip",
            "base64",
            "openssl",
            "tee",
            "eval",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        Self {
            rules: Vec::new(),
            known_safe,
            known_dangerous,
        }
    }

    pub fn add_rule(&mut self, rule: ExecRule) {
        self.rules.push(rule);
    }

    pub fn add_rules(&mut self, rules: Vec<ExecRule>) {
        self.rules.extend(rules);
    }

    pub fn load_rules_from_str(&mut self, content: &str) -> Result<(), String> {
        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let rule = self
                .parse_rule_line(trimmed)
                .map_err(|e| format!("Line {}: {}", line_num + 1, e))?;
            self.rules.push(rule);
        }
        Ok(())
    }

    pub fn load_rules_from_file(&mut self, path: &Path) -> Result<(), String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read rules file '{}': {}", path.display(), e))?;
        self.load_rules_from_str(&content)
    }

    pub fn merge(&self, other: &ExecPolicy) -> ExecPolicy {
        let mut merged = self.clone();
        merged.rules.extend(other.rules.clone());
        merged
    }

    pub fn evaluate(&self, command: &str) -> ExecDecision {
        let trimmed = command.trim();
        if trimmed.is_empty() {
            return ExecDecision::Prompt;
        }

        // Security: detect shell meta characters (;, &&, ||, |, `, $())
        // that could extend a command beyond what the prefix-matching rule checked.
        // E.g. rule "git push" matches "git push origin main; rm -rf /" via token
        // prefix matching, but the shell executes BOTH commands.
        let has_shell_meta = contains_shell_meta(trimmed);

        // First pass: match specific (non-catch-all) rules. Last match wins.
        // Standalone `*` patterns are treated as catch-all defaults and only
        // apply when no specific rule matches (avoiding the issue where a
        // `* -> prompt` at the end overrides all preceding allow rules).
        let mut decision: Option<ExecDecision> = None;
        let mut catch_all: Option<ExecDecision> = None;
        for rule in &self.rules {
            if rule.pattern.trim() == "*" {
                catch_all = Some(rule.decision);
                continue;
            }
            if command_starts_with_pattern(trimmed, &rule.pattern) {
                decision = Some(rule.decision);
            }
        }

        // If a specific rule matched, use it
        if let Some(d) = decision {
            // If the command has shell meta characters, upgrade Allow to Prompt
            // unless there's an explicit rule covering the full command.
            if d == ExecDecision::Allow && has_shell_meta {
                return ExecDecision::Prompt;
            }
            return d;
        }

        // Fallback to catch-all * rule if no specific rule matched
        if let Some(d) = catch_all {
            if d == ExecDecision::Allow && has_shell_meta {
                return ExecDecision::Prompt;
            }
            return d;
        }

        // Heuristic fallback
        let first_word = trimmed
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_lowercase();

        // If the first command has shell meta characters, always prompt
        if has_shell_meta {
            return ExecDecision::Prompt;
        }

        if self.known_dangerous.contains(&first_word) {
            ExecDecision::Prompt
        } else if self.known_safe.contains(&first_word) {
            ExecDecision::Allow
        } else {
            ExecDecision::Prompt
        }
    }

    pub fn is_allowed(&self, command: &str) -> bool {
        self.evaluate(command) == ExecDecision::Allow
    }

    pub fn is_forbidden(&self, command: &str) -> bool {
        self.evaluate(command) == ExecDecision::Forbidden
    }

    fn parse_rule_line(&self, line: &str) -> Result<ExecRule, String> {
        let arrow_pos = line.find("->").ok_or_else(|| {
            format!(
                "Invalid rule syntax '{}': expected 'pattern -> decision'",
                line
            )
        })?;

        let pattern = line[..arrow_pos].trim().to_string();
        let decision_str = line[arrow_pos + 2..].trim().to_lowercase();

        let decision = match decision_str.as_str() {
            "allow" => ExecDecision::Allow,
            "prompt" => ExecDecision::Prompt,
            "forbid" | "forbidden" => ExecDecision::Forbidden,
            _ => {
                return Err(format!(
                    "Invalid decision '{}' in rule '{}'. Expected 'allow', 'prompt', or 'forbid'",
                    decision_str, line
                ));
            }
        };

        Ok(ExecRule {
            pattern,
            decision,
            justification: None,
        })
    }
}

fn extract_command_prefix(command: &str) -> String {
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return String::new();
    }

    let first = parts[0].to_lowercase();

    let compound_commands = [
        "git", "npm", "cargo", "docker", "kubectl", "gh", "npx", "bun", "yarn", "dotnet", "make",
        "pip", "rustup", "go", "winget", "choco", "apt", "apt-get", "brew", "pacman", "snap",
    ];

    if compound_commands.contains(&first.as_str()) && parts.len() > 1 {
        format!("{} {}", first, parts[1].to_lowercase())
    } else {
        first
    }
}

/// Check if a command starts with the given pattern (token-aware).
/// E.g. pattern "git push" matches command "git push origin main"
/// but not "git pusher" or "git".
fn command_starts_with_pattern(command: &str, pattern: &str) -> bool {
    let cmd_tokens: Vec<&str> = command.split_whitespace().collect();
    let pat_tokens: Vec<&str> = pattern.split_whitespace().collect();

    if pat_tokens.is_empty() {
        return true;
    }

    if cmd_tokens.len() < pat_tokens.len() {
        return false;
    }

    // Support wildcard in the last position: "git *" matches any git subcommand
    if pat_tokens.last() == Some(&"*") {
        let base = &pat_tokens[..pat_tokens.len() - 1];
        if cmd_tokens.len() < base.len() {
            return false;
        }
        return cmd_tokens[..base.len()]
            .iter()
            .zip(base.iter())
            .all(|(a, b)| a.eq_ignore_ascii_case(b));
    }

    cmd_tokens[..pat_tokens.len()]
        .iter()
        .zip(pat_tokens.iter())
        .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

/// Detect shell meta characters that can extend or chain commands.
/// When present, token-prefix matching is unreliable because the command
/// contains multiple sub-commands.
fn contains_shell_meta(command: &str) -> bool {
    // Semicolons: cmd1; cmd2
    if command.contains(';') {
        return true;
    }
    // AND/OR chaining: cmd1 && cmd2, cmd1 || cmd2
    if command.contains("&&") || command.contains("||") {
        return true;
    }
    // Pipes: cmd1 | cmd2
    if command.contains('|') {
        return true;
    }
    // Command substitution: $(cmd), `cmd`
    if command.contains("$(") || command.contains('`') {
        return true;
    }
    false
}

fn wildcard_match(pattern: &str, input: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_prefix_simple() {
        assert_eq!(extract_command_prefix("ls -la"), "ls");
        assert_eq!(extract_command_prefix("cat file.txt"), "cat");
    }

    #[test]
    fn test_extract_prefix_compound() {
        assert_eq!(extract_command_prefix("git push origin main"), "git push");
        assert_eq!(extract_command_prefix("npm install --save"), "npm install");
        assert_eq!(
            extract_command_prefix("cargo build --release"),
            "cargo build"
        );
    }

    #[test]
    fn test_evaluate_compound_allowed() {
        let policy = ExecPolicy::new();
        // Without rules, safe commands should be Allow
        assert_eq!(policy.evaluate("ls -la"), ExecDecision::Allow);
        assert_eq!(policy.evaluate("cat file.txt"), ExecDecision::Allow);
    }

    #[test]
    fn test_evaluate_unknown_is_prompt() {
        let policy = ExecPolicy::new();
        assert_eq!(
            policy.evaluate("some_random_tool --flag"),
            ExecDecision::Prompt
        );
    }

    #[test]
    fn test_evaluate_dangerous_is_prompt() {
        let policy = ExecPolicy::new();
        assert_eq!(policy.evaluate("rm -rf /"), ExecDecision::Prompt);
        assert_eq!(policy.evaluate("sudo rm -rf"), ExecDecision::Prompt);
    }

    #[test]
    fn test_rule_file_parse() {
        let content = r#"
# Read-only commands
ls -> allow
cat -> allow
git diff -> allow

# Dangerous
rm -rf -> forbid
git push -> prompt
        "#;

        let policy = ExecPolicy::new();
        let mut policy = policy;
        policy.load_rules_from_str(content).unwrap();

        assert_eq!(policy.evaluate("ls"), ExecDecision::Allow);
        assert_eq!(policy.evaluate("cat /etc/passwd"), ExecDecision::Allow);
        assert_eq!(policy.evaluate("git diff HEAD"), ExecDecision::Allow);
        assert_eq!(policy.evaluate("rm -rf /"), ExecDecision::Forbidden);
        assert_eq!(
            policy.evaluate("git push origin main"),
            ExecDecision::Prompt
        );
    }

    #[test]
    fn test_last_match_wins() {
        let mut policy = ExecPolicy::new();
        policy
            .load_rules_from_str(
                "
git -> forbid
git push -> allow
        ",
            )
            .unwrap();

        assert_eq!(policy.evaluate("git status"), ExecDecision::Forbidden);
        assert_eq!(policy.evaluate("git push"), ExecDecision::Allow);
    }

    #[test]
    fn test_wildcard_catch_all_is_fallback() {
        let mut policy = ExecPolicy::new();
        policy
            .load_rules_from_str(
                "
npm * -> allow
* -> prompt
        ",
            )
            .unwrap();

        // `npm *` is a specific pattern, so it matches first.
        // `* -> prompt` is now a catch-all fallback, only applies if no other rule matched.
        assert_eq!(policy.evaluate("npm install"), ExecDecision::Allow);
    }

    #[test]
    fn test_catch_all_fallback_unknown_command() {
        let mut policy = ExecPolicy::new();
        policy
            .load_rules_from_str(
                "
git log -> allow
* -> prompt
        ",
            )
            .unwrap();

        // Known command with specific allow rule → Allow
        assert_eq!(policy.evaluate("git log --oneline"), ExecDecision::Allow);
        // Unknown command with no specific rule → falls back to * -> prompt
        assert_eq!(policy.evaluate("some_random_tool"), ExecDecision::Prompt);
    }

    #[test]
    fn test_dangerous_heuristic() {
        let policy = ExecPolicy::new();
        // Even without rules, rm is known dangerous
        assert_eq!(policy.evaluate("rm -rf /"), ExecDecision::Prompt);
        // ls is known safe
        assert_eq!(policy.evaluate("ls -la"), ExecDecision::Allow);
    }

    #[test]
    fn test_heuristic_fallback() {
        let policy = ExecPolicy::new();
        assert_eq!(policy.evaluate("ls"), ExecDecision::Allow);
        assert_eq!(policy.evaluate("pwd"), ExecDecision::Allow);
    }

    #[test]
    fn test_empty_command() {
        let policy = ExecPolicy::new();
        // Empty command should prompt since it's an unusual/unexpected request
        assert_eq!(policy.evaluate(""), ExecDecision::Prompt);
    }
}
