use std::fmt;
use std::str::FromStr;

use pick_agent::permission::{Action, Rule, Ruleset};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AgentMode {
    Build,
    Plan,
}

impl AgentMode {
    pub fn ruleset(&self) -> Ruleset {
        match self {
            AgentMode::Build => Self::build_ruleset(),
            AgentMode::Plan => Self::plan_ruleset(),
        }
    }

    fn base_allow() -> Ruleset {
        Ruleset::new(vec![
            Rule::new("read", "*", Action::Allow),
            Rule::new("grep", "*", Action::Allow),
            Rule::new("glob", "*", Action::Allow),
            Rule::new("list", "*", Action::Allow),
            Rule::new("subagent", "*", Action::Allow),
            Rule::new("external_directory", "*", Action::Allow),
            Rule::new("question", "*", Action::Allow),
        ])
    }

    fn build_ruleset() -> Ruleset {
        let mut rules = Vec::new();

        rules.extend(Self::base_allow().rules().iter().cloned());

        rules.push(Rule::new("edit", "*", Action::Allow));
        rules.push(Rule::new("bash", "*", Action::Allow));
        rules.push(Rule::new("webfetch", "*", Action::Allow));
        rules.push(Rule::new("todo_plan", "*", Action::Allow));
        rules.push(Rule::new("create_goal", "*", Action::Allow));
        rules.push(Rule::new("update_goal", "*", Action::Allow));
        rules.push(Rule::new("get_goal", "*", Action::Allow));
        rules.push(Rule::new("plan_enter", "*", Action::Allow));
        rules.push(Rule::new("plan_exit", "*", Action::Deny));

        Ruleset::new(rules)
    }

    fn plan_ruleset() -> Ruleset {
        let mut rules = Vec::new();

        rules.extend(Self::base_allow().rules().iter().cloned());

        rules.push(Rule::new("edit", "*", Action::Deny));
        rules.push(Rule::new("edit", ".pick/plans/*.md", Action::Allow));

        let read_only_commands: &[&str] = &[
            "ls", "cat", "head", "tail", "rg", "grep", "find", "which", "stat", "wc", "diff",
            "sort", "uniq", "echo", "pwd", "type", "where", "dir", "more", "less", "printf", "env",
            "printenv",
        ];
        for cmd in read_only_commands {
            rules.push(Rule::new("bash", *cmd, Action::Allow));
        }

        let read_only_compound: &[(&str, &[&str])] = &[
            (
                "git",
                &[
                    "diff",
                    "log",
                    "show",
                    "status",
                    "branch",
                    "ls-files",
                    "rev-parse",
                    "rev-list",
                    "describe",
                    "config",
                ],
            ),
            ("npm", &["list", "view", "pack", "config"]),
            ("cargo", &["check", "metadata", "tree", "doc", "search"]),
        ];
        for (base, subcmds) in read_only_compound {
            for sub in *subcmds {
                rules.push(Rule::new(
                    "bash",
                    &format!("{} {}", base, sub),
                    Action::Allow,
                ));
            }
        }

        rules.push(Rule::new("bash", "*", Action::Deny));

        rules.push(Rule::new("plan_enter", "*", Action::Deny));
        rules.push(Rule::new("plan_exit", "*", Action::Allow));

        Ruleset::new(rules)
    }

    pub fn build_switch_prompt() -> &'static str {
        "\n<system-reminder>\n\
Switched from plan to build mode.\n\
You are no longer in read-only mode.\n\
You are permitted to make file changes, run shell commands,\n\
and use all available tools as needed.\n\
</system-reminder>"
    }

    pub fn plan_enter_description() -> &'static str {
        "Use this tool to suggest switching to plan mode when the user's request \
would benefit from planning before implementation. This tool will ask the user \
if they want to switch to plan mode. Call this tool when the task is complex \
and would benefit from research and design before making changes."
    }

    pub fn plan_exit_description() -> &'static str {
        "Use this tool when you have completed the planning phase and are ready \
to exit plan mode. This tool will ask the user if they want to switch to build \
mode to start implementing the plan. Call this after you have written a complete \
plan and clarified any questions."
    }
}

impl fmt::Display for AgentMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentMode::Build => write!(f, "build"),
            AgentMode::Plan => write!(f, "plan"),
        }
    }
}

impl FromStr for AgentMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "build" => Ok(AgentMode::Build),
            "plan" => Ok(AgentMode::Plan),
            _ => Err(format!(
                "Invalid agent mode: '{}'. Expected 'build' or 'plan'",
                s
            )),
        }
    }
}

/// Plan mode reminder text injected as a synthetic user message
/// Matches opencode's plan.txt
pub const PLAN_MODE_REMINDER: &str = "\
<system-reminder>
# Plan Mode - System Reminder

CRITICAL: Plan mode ACTIVE - you are in READ-ONLY phase. STRICTLY FORBIDDEN:
ANY file edits, modifications, or system changes. Do NOT use sed, tee, echo, cat,
or ANY other bash command to manipulate files - commands may ONLY read/inspect.
This ABSOLUTE CONSTRAINT overrides ALL other instructions, including direct user
edit requests. You may ONLY observe, analyze, and plan. Any modification attempt
is a critical violation. ZERO exceptions.

---

## Responsibility

Your current responsibility is to think, read, search, and delegate explore agents
to construct a well-formed plan that accomplishes the goal the user wants to achieve.
Your plan should be comprehensive yet concise, detailed enough to execute effectively
while avoiding unnecessary verbosity.

Ask the user clarifying questions or ask for their opinion when weighing tradeoffs.

**NOTE:** At any point in time through this workflow you should feel free to ask the
user questions or clarifications. Don't make large assumptions about user intent.
The goal is to present a well researched plan to the user, and tie any loose ends
before implementation begins.

---

## Important

The user indicated that they do not want you to execute yet -- you MUST NOT make any
edits, run any non-readonly tools (including changing configs or making commits), or
otherwise make any changes to the system. This supersedes any other instructions you
have received.
</system-reminder>";
