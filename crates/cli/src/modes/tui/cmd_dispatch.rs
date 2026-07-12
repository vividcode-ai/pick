use super::cmd_core;
use super::cmd_goal;
use super::cmd_init;
use super::cmd_io;
use super::cmd_loop;
use super::cmd_mcp;
use super::cmd_mgmt;
use super::cmd_model_login;
use super::context::TuiContext;

/// Handle a slash command.
pub(crate) async fn handle_slash_command(
    ctx: &mut TuiContext,
    cmd_name: &str,
    args: &[String],
    _user_text: &str,
) -> SlashCommandResult {
    match cmd_name {
        "help" => {
            cmd_core::handle_help(ctx);
            SlashCommandResult::Consumed
        }
        "skill" => {
            cmd_core::handle_skill_list(ctx);
            SlashCommandResult::Consumed
        }
        "quit" => {
            cmd_core::handle_quit(ctx).await;
            SlashCommandResult::Quit
        }
        "session" | "info" => {
            cmd_core::handle_session_info(ctx);
            SlashCommandResult::Consumed
        }
        "name" => {
            cmd_core::handle_session_name(ctx, &args.join(" ")).await;
            SlashCommandResult::Consumed
        }
        "plan" | "plan_enter" => {
            cmd_core::handle_plan(ctx).await;
            SlashCommandResult::Consumed
        }
        "build" | "plan_exit" => {
            cmd_core::handle_build(ctx).await;
            SlashCommandResult::Consumed
        }
        "changelog" => {
            cmd_core::handle_changelog(ctx);
            SlashCommandResult::Consumed
        }
        "hotkeys" => {
            cmd_core::handle_hotkeys(ctx);
            SlashCommandResult::Consumed
        }
        "export" => {
            cmd_io::handle_export(ctx, args).await;
            SlashCommandResult::Consumed
        }
        "import" => {
            cmd_io::handle_import(ctx, args).await;
            SlashCommandResult::Consumed
        }
        "share" => {
            cmd_io::handle_share(ctx).await;
            SlashCommandResult::Consumed
        }
        "copy" => {
            cmd_io::handle_copy(ctx);
            SlashCommandResult::Consumed
        }
        "fork" => {
            cmd_mgmt::handle_fork_selector(ctx);
            SlashCommandResult::Consumed
        }
        "clone" => {
            super::actions_session::handle_clone(ctx).await;
            SlashCommandResult::Consumed
        }
        "tree" => {
            cmd_mgmt::handle_tree_command(ctx, args);
            SlashCommandResult::Consumed
        }
        "resume" => {
            cmd_mgmt::handle_resume_selector(ctx);
            SlashCommandResult::Consumed
        }
        "compact" => {
            cmd_mgmt::handle_compact(ctx, args).await;
            SlashCommandResult::Consumed
        }
        "new" => {
            super::actions_session::handle_new_session(ctx).await;
            SlashCommandResult::Consumed
        }
        "reload" => {
            cmd_mgmt::handle_reload(ctx).await;
            SlashCommandResult::Consumed
        }
        "goal" => {
            let should_add = cmd_goal::handle_goal(ctx, args).await;
            if should_add {
                SlashCommandResult::Consumed
            } else {
                SlashCommandResult::ContinueSubmit
            }
        }
        "init" => {
            let should_add = cmd_init::handle_init(ctx, args).await;
            if should_add {
                SlashCommandResult::Consumed
            } else {
                SlashCommandResult::ContinueSubmit
            }
        }
        "mcp" => {
            cmd_mcp::handle_mcp(ctx, args).await;
            SlashCommandResult::Consumed
        }
        "model" => {
            cmd_model_login::handle_model_command(ctx, args);
            SlashCommandResult::Consumed
        }
        "scoped-models" => {
            cmd_model_login::handle_scoped_models_command(ctx);
            SlashCommandResult::Consumed
        }
        "settings" => {
            cmd_model_login::handle_settings_command(ctx);
            SlashCommandResult::Consumed
        }
        "connect" => {
            cmd_model_login::handle_login_command(ctx);
            SlashCommandResult::Consumed
        }
        "unconnect" => {
            cmd_model_login::handle_logout_command(ctx);
            SlashCommandResult::Consumed
        }
        "loop" | "loop-goal" | "loop-status" | "loop-pause" | "loop-resume" | "loop-remove"
        | "loop-clear" | "loop-now" | "loop-stop" | "loop-help" | "loop-ask" | "loop-command"
        | "loop-cmd" | "loop-shell" | "loop-goal-status" | "loop-goal-pause"
        | "loop-goal-resume" | "loop-goal-clear" | "loop-goal-done" | "loop-goal-complete"
        | "loop-goal-blocked" => {
            cmd_loop::handle_loop(ctx, cmd_name, args).await;
            SlashCommandResult::Consumed
        }
        _ => {
            ctx.tui.chat.add_system_message(&format!(
                "Unknown command: \x1b[1m/{}\x1b[0m. Type \x1b[1m/help\x1b[0m for available commands.",
                cmd_name
            ));
            SlashCommandResult::Consumed
        }
    }
}

/// Expand /skill:name command into XML block
pub(crate) fn expand_skill_command(ctx: &mut TuiContext, user_text: &str) -> Option<String> {
    let sm = crate::core::settings::SettingsManager::load(&ctx.cwd);
    if !sm.get_enable_skill_commands() {
        ctx.tui
            .chat
            .add_system_message("Skill commands are disabled. Enable them in settings.");
        ctx.tui.finalize_turn();
        return None;
    }
    let trimmed = user_text.trim();
    let skill_name = trimmed[7..]
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string();
    match crate::utils::frontmatter::expand_skill_command(trimmed, ctx.resource_loader.skills()) {
        Some(expanded) => {
            if let Some(skill) = ctx
                .resource_loader
                .skills()
                .iter()
                .find(|s| s.name == skill_name)
            {
                ctx.tui
                    .chat
                    .add_system_message(&format!("[Skill: {}] {}", skill.name, skill.description));
            }
            Some(expanded)
        }
        None => {
            ctx.tui.chat.add_system_message(&format!(
                "Unknown skill: {}. Type /help to see available skills.",
                skill_name
            ));
            ctx.tui.finalize_turn();
            None
        }
    }
}

/// Result of handling a slash command
pub(crate) enum SlashCommandResult {
    Consumed,
    ContinueSubmit,
    Quit,
}
