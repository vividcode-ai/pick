use pick_ai::types::{Message, UserMessage};

use super::context::TuiContext;

/// Handle /init slash command — generate AGENTS.md via AI.
///
/// Loads init_prompt.txt, substitutes ${path} with the working directory,
/// optionally appends user-provided arguments as additional context,
/// then injects the prompt as a user message to drive the agent.
pub(crate) async fn handle_init(ctx: &mut TuiContext, args: &[String]) -> bool {
    let cwd_str = ctx.cwd.to_string_lossy();
    let args_str = args.join(" ");

    // Load template and substitute ${path}
    let template = include_str!("init_prompt.txt").replace("${path}", &cwd_str);

    // Append user-provided context if any
    let prompt = if args_str.trim().is_empty() {
        template
    } else {
        format!(
            "{}\n\n## Additional context\n\n{}",
            template,
            args_str.trim()
        )
    };

    // Show user feedback
    ctx.tui
        .chat
        .add_system_message("\x1b[36m→\x1b[0m \x1b[1mInitializing AGENTS.md...\x1b[0m");

    // Inject into agent messages (not shown in chat UI)
    ctx.all_messages
        .push(Message::User(UserMessage::text(&prompt)));

    // Return false → ContinueSubmit: raw "/init" not added as a user message
    false
}
