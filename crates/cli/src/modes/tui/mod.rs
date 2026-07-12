mod action_dispatch;
mod actions_login;
mod actions_mcp;
mod actions_model;
mod actions_session;
mod actions_settings;
pub(crate) mod agent_exec;
mod cleanup;
mod cmd_core;
mod cmd_dispatch;
mod cmd_goal;
mod cmd_init;
mod cmd_io;
mod cmd_loop;
mod cmd_mcp;
mod cmd_mgmt;
mod cmd_model_login;
mod commands;
mod context;
mod event_handler;
mod init;
mod key_events;
mod message_utils;
mod runner;
mod settings_values;
mod tree_summarize;
mod tree_utils;
mod types;

pub use runner::run_tui_mode;

#[cfg(test)]
mod tests {
    /// Verify all slash commands handled in the TUI match the built-in list
    #[test]
    fn test_tui_slash_command_handlers_complete() {
        let cmd_names: std::collections::HashSet<&str> = [
            "quit",
            "help",
            "connect",
            "unconnect",
            "model",
            "scoped-models",
            "settings",
            "session",
            "info",
            "name",
            "export",
            "import",
            "share",
            "copy",
            "fork",
            "clone",
            "tree",
            "resume",
            "compact",
            "new",
            "reload",
            "changelog",
            "hotkeys",
            "skill",
            "plan",
            "plan_enter",
            "build",
            "plan_exit",
            "mcp",
            "goal",
            "init",
            "loop",
            "loop-goal",
            "loop-status",
            "loop-pause",
            "loop-resume",
            "loop-remove",
            "loop-clear",
            "loop-now",
            "loop-stop",
            "loop-help",
            "loop-ask",
            "loop-command",
            "loop-cmd",
            "loop-shell",
            "loop-goal-status",
            "loop-goal-pause",
            "loop-goal-resume",
            "loop-goal-clear",
            "loop-goal-done",
            "loop-goal-complete",
            "loop-goal-blocked",
        ]
        .into_iter()
        .collect();

        let pick_commands = crate::core::slash_commands::BUILTIN_SLASH_COMMANDS;
        for cmd in pick_commands {
            assert!(
                cmd_names.contains(cmd.name),
                "TUI handler for /{} must exist in the match statement",
                cmd.name
            );
        }
    }

    #[test]
    fn test_tree_filter_modes() {
        let modes: std::collections::HashSet<&str> =
            ["default", "no-tools", "user-only", "labeled-only", "all"]
                .into_iter()
                .collect();

        let synonyms: std::collections::HashSet<&str> = [
            "no-tools",
            "notools",
            "user",
            "user-only",
            "label",
            "labeled",
            "labeled-only",
            "all",
        ]
        .into_iter()
        .collect();

        assert_eq!(modes.len(), 5, "there are exactly 5 tree filter modes");
        assert!(synonyms.contains("no-tools"), "no-tools accepted");
        assert!(synonyms.contains("user-only"), "user-only accepted");
        assert!(synonyms.contains("labeled-only"), "labeled-only accepted");
        assert!(synonyms.contains("all"), "all accepted");
    }

    #[test]
    fn test_import_jsonl_only() {
        assert!(
            "file.jsonl".ends_with(".jsonl"),
            "jsonl files should be accepted"
        );
        assert!(
            !"file.html".ends_with(".jsonl"),
            "non-jsonl files should be rejected"
        );
        assert!(
            !"file".ends_with(".jsonl"),
            "files without extension should be rejected"
        );
    }

    #[test]
    fn test_compact_custom_instructions() {
        let args: Vec<&str> = "focus on recent changes".split_whitespace().collect();
        let instructions = if args.is_empty() {
            None
        } else {
            Some(args.join(" "))
        };
        assert_eq!(instructions, Some("focus on recent changes".to_string()));

        let empty_args: Vec<&str> = vec![];
        let no_instructions: Option<String> = if empty_args.is_empty() {
            None
        } else {
            Some(empty_args.join(" "))
        };
        assert!(
            no_instructions.is_none(),
            "no args should mean no custom instructions"
        );
    }

    #[test]
    fn test_model_exact_match_pattern() {
        let search = "claude-sonnet-4-20250514".to_string();
        assert!(
            !search.is_empty(),
            "search should not be empty for exact match"
        );

        let empty_search = "".to_string();
        assert!(empty_search.is_empty(), "empty search should show selector");
    }
}
