pub mod approval;
pub mod audit;
pub mod disabled;
pub mod evaluate;
pub mod exec_policy;
pub mod external_dir;
pub mod fs_policy;
pub mod guardian;
pub mod hooks;
pub mod manager;
pub mod network;
pub mod profiles;
pub mod sandbox;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Action {
    #[serde(rename = "allow")]
    Allow,
    #[serde(rename = "deny")]
    Deny,
    #[serde(rename = "ask")]
    Ask,
}

impl Action {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Action::Allow)
    }

    pub fn is_denied(&self) -> bool {
        matches!(self, Action::Deny)
    }

    pub fn is_ask(&self) -> bool {
        matches!(self, Action::Ask)
    }
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub permission: String,
    pub pattern: String,
    pub action: Action,
}

impl Rule {
    pub fn new(permission: impl Into<String>, pattern: impl Into<String>, action: Action) -> Self {
        Self {
            permission: permission.into(),
            pattern: pattern.into(),
            action,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Ruleset {
    rules: Vec<Rule>,
}

impl Ruleset {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn merge(&self, other: &Ruleset) -> Ruleset {
        let mut merged = self.rules.clone();
        merged.extend(other.rules.clone());
        Ruleset { rules: merged }
    }
}

impl From<Vec<Rule>> for Ruleset {
    fn from(rules: Vec<Rule>) -> Self {
        Self { rules }
    }
}

pub fn action_from_str(s: &str) -> Option<Action> {
    match s {
        "allow" => Some(Action::Allow),
        "deny" => Some(Action::Deny),
        "ask" => Some(Action::Ask),
        _ => None,
    }
}

pub fn action_to_str(a: Action) -> &'static str {
    match a {
        Action::Allow => "allow",
        Action::Deny => "deny",
        Action::Ask => "ask",
    }
}

pub const PERMISSION_KEYS: &[&str] = &[
    "read",
    "edit",
    "bash",
    "grep",
    "glob",
    "list",
    "subagent",
    "question",
    "plan_enter",
    "plan_exit",
    "external_directory",
    "webfetch",
    "todo_plan",
    "create_goal",
    "update_goal",
    "get_goal",
];

pub fn tool_to_permission_key(tool_name: &str) -> &str {
    match tool_name {
        "read" => "read",
        "write" | "edit" | "apply_patch" | "multiedit" => "edit",
        "bash" => "bash",
        "grep" => "grep",
        "glob" | "find" => "glob",
        "ls" => "list",
        "subagent" | "task" => "subagent",
        "question" => "question",
        "webfetch" => "webfetch",
        "todo_plan" => "todo_plan",
        "create_goal" => "create_goal",
        "update_goal" => "update_goal",
        "get_goal" => "get_goal",
        "plan_enter" => "plan_enter",
        "plan_exit" => "plan_exit",
        "external_directory" => "external_directory",
        _ => tool_name,
    }
}
