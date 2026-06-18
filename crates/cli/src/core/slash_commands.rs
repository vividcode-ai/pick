use crate::core::source_info::SourceInfo;

/// Source of a slash command
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommandSource {
    Extension,
    Prompt,
    Skill,
}

impl std::fmt::Display for SlashCommandSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SlashCommandSource::Extension => write!(f, "extension"),
            SlashCommandSource::Prompt => write!(f, "prompt"),
            SlashCommandSource::Skill => write!(f, "skill"),
        }
    }
}

/// Information about a slash command
#[derive(Debug, Clone)]
pub struct SlashCommandInfo {
    pub name: String,
    pub description: Option<String>,
    pub source: SlashCommandSource,
    pub source_info: SourceInfo,
}

/// A built-in slash command definition
#[derive(Debug, Clone)]
pub struct BuiltinSlashCommand {
    pub name: &'static str,
    pub description: &'static str,
}

/// Built-in slash commands available in the application
pub const BUILTIN_SLASH_COMMANDS: &[BuiltinSlashCommand] = &[
    BuiltinSlashCommand {
        name: "settings",
        description: "Open settings menu",
    },
    BuiltinSlashCommand {
        name: "model",
        description: "Select model (opens selector UI)",
    },
    BuiltinSlashCommand {
        name: "scoped-models",
        description: "Enable/disable models for Ctrl+P cycling",
    },
    BuiltinSlashCommand {
        name: "export",
        description: "Export session (HTML default, or specify path: .html/.jsonl)",
    },
    BuiltinSlashCommand {
        name: "import",
        description: "Import and resume a session from a JSONL file",
    },
    BuiltinSlashCommand {
        name: "share",
        description: "Share session as a secret GitHub gist",
    },
    BuiltinSlashCommand {
        name: "copy",
        description: "Copy last agent message to clipboard",
    },
    BuiltinSlashCommand {
        name: "name",
        description: "Set session display name",
    },
    BuiltinSlashCommand {
        name: "session",
        description: "Show session info and stats",
    },
    BuiltinSlashCommand {
        name: "changelog",
        description: "Show changelog entries",
    },
    BuiltinSlashCommand {
        name: "hotkeys",
        description: "Show all keyboard shortcuts",
    },
    BuiltinSlashCommand {
        name: "fork",
        description: "Create a new fork from a previous user message",
    },
    BuiltinSlashCommand {
        name: "clone",
        description: "Duplicate the current session at the current position",
    },
    BuiltinSlashCommand {
        name: "tree",
        description: "Navigate session tree (switch branches)",
    },
    BuiltinSlashCommand {
        name: "login",
        description: "Configure provider authentication",
    },
    BuiltinSlashCommand {
        name: "logout",
        description: "Remove provider authentication",
    },
    BuiltinSlashCommand {
        name: "new",
        description: "Start a new session",
    },
    BuiltinSlashCommand {
        name: "compact",
        description: "Manually compact the session context",
    },
    BuiltinSlashCommand {
        name: "resume",
        description: "Resume a different session",
    },
    BuiltinSlashCommand {
        name: "reload",
        description: "Reload keybindings, extensions, skills, prompts, and themes",
    },
    BuiltinSlashCommand {
        name: "plan",
        description: "Switch to plan mode (read-only, no edits)",
    },
    BuiltinSlashCommand {
        name: "build",
        description: "Switch to build mode (full tool access)",
    },
    BuiltinSlashCommand {
        name: "plan_exit",
        description: "Complete planning and switch to build mode",
    },
    BuiltinSlashCommand {
        name: "quit",
        description: "Quit the application",
    },
    BuiltinSlashCommand {
        name: "skill",
        description: "List available skills (/skill:<name> to load a skill)",
    },
    BuiltinSlashCommand {
        name: "mcp",
        description: "Manage MCP server connections (list | connect | disconnect)",
    },
    BuiltinSlashCommand {
        name: "goal",
        description: "Set or show a persistent goal. /goal <text> to create, /goal edit|pause|resume|clear",
    },
];

/// New commands added beyond the original built-in list
const _NEW_SLASH_COMMANDS: &[&str] = &["plan", "build", "plan_exit"];


#[cfg(test)]
mod tests {
    use super::*;

    const PICK_ORIGINAL_COMMANDS: &[&str] = &[
        "settings", "model", "scoped-models", "export", "import",
        "share", "copy", "name", "session", "changelog",
        "hotkeys", "fork", "clone", "tree", "login",
        "logout", "new", "compact", "resume", "reload", "quit",
        "plan", "build", "plan_exit", "skill", "mcp",
    ];

    #[test]
    fn test_all_builtin_commands() {
        let mut pick_names: Vec<&str> = BUILTIN_SLASH_COMMANDS.iter().map(|c| c.name).collect();
        pick_names.sort();
        let mut original_names = PICK_ORIGINAL_COMMANDS.to_vec();
        original_names.sort();

        for name in &["plan", "build", "plan_exit", "goal"] {
            assert!(pick_names.contains(name), "must contain new command {}", name);
        }
        // Original commands minus any that have been split into separate commands
        for name in &original_names {
            assert!(pick_names.contains(name), "must contain command {}", name);
        }
    }

    #[test]
    fn test_exactly_27_commands() {
        assert_eq!(
            BUILTIN_SLASH_COMMANDS.len(),
            27,
            "must have exactly 27 built-in slash commands"
        );
    }

    #[test]
    fn test_every_command_has_name_and_description() {
        for cmd in BUILTIN_SLASH_COMMANDS {
            assert!(!cmd.name.is_empty(), "command name must not be empty");
            assert!(!cmd.description.is_empty(), "command /{} must have a description", cmd.name);
        }
    }

    #[test]
    fn test_no_duplicate_names() {
        let mut names: Vec<&str> = BUILTIN_SLASH_COMMANDS.iter().map(|c| c.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(
            names.len(),
            BUILTIN_SLASH_COMMANDS.len(),
            "there should be no duplicate command names"
        );
    }
}
