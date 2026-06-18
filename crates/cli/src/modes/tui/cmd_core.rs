use pick_agent::session::SessionEntry;

use super::context::TuiContext;
use super::init;

/// Handle /help slash command
pub(crate) fn handle_help(ctx: &mut TuiContext) {
    ctx.tui.chat.add_system_message("Available commands:");
    ctx.tui.chat.add_system_message("");
    let all_commands = crate::core::slash_commands::BUILTIN_SLASH_COMMANDS;
    for cmd in all_commands {
        let desc = cmd.description;
        ctx.tui
            .chat
            .add_system_message(&format!("  /{:<20} {}", cmd.name, desc));
    }
    let sm = crate::core::settings::SettingsManager::load(&ctx.cwd);
    if sm.get_enable_skill_commands() {
        let skills = ctx.resource_loader.skills();
        if !skills.is_empty() {
            ctx.tui.chat.add_system_message("");
            ctx.tui.chat.add_system_message("Skills:");
            for skill in skills {
                ctx.tui
                    .chat
                    .add_system_message(&format!("  /skill:{:<14} {}", skill.name, skill.description));
            }
        }
    }
}

/// Handle bare /skill command — show available skills
pub(crate) fn handle_skill_list(ctx: &mut TuiContext) {
    let sm = crate::core::settings::SettingsManager::load(&ctx.cwd);
    if sm.get_enable_skill_commands() {
        let skills = ctx.resource_loader.skills();
        if skills.is_empty() {
            ctx.tui.chat.add_system_message("No skills loaded. Place SKILL.md files in:");
            ctx.tui
                .chat
                .add_system_message(&format!("  {}/skills/", crate::config::get_agent_dir().display()));
            ctx.tui.chat.add_system_message("  .pick/skills/");
        } else {
            ctx.tui
                .chat
                .add_system_message(&format!("Available skills ({}):", skills.len()));
            for skill in skills {
                ctx.tui
                    .chat
                    .add_system_message(&format!("  /skill:{:<14} {}", skill.name, skill.description));
            }
        }
    } else {
        ctx.tui
            .chat
            .add_system_message("Skill commands are disabled. Enable them in settings.");
    }
}

/// Handle /session or /info slash command
pub(crate) fn handle_session_info(ctx: &mut TuiContext) {
    let id = ctx
        .session_manager
        .header()
        .map(|h| h.id.as_str())
        .unwrap_or("none");
    let msg_count = ctx.all_messages.len();
    let user_msgs = ctx
        .all_messages
        .iter()
        .filter(|m| matches!(m, pick_ai::types::Message::User(_)))
        .count();
    let assistant_msgs = ctx
        .all_messages
        .iter()
        .filter(|m| matches!(m, pick_ai::types::Message::Assistant(_)))
        .count();
    let tool_msgs = ctx
        .all_messages
        .iter()
        .filter(|m| matches!(m, pick_ai::types::Message::ToolResult(_)))
        .count();
    let name_display = ctx.tui.session_name.as_deref().unwrap_or("(none)");

    let mut total_input: u64 = 0;
    let mut total_output: u64 = 0;
    let mut total_cache_read: u64 = 0;
    let mut total_cache_write: u64 = 0;
    let mut total_cost: f64 = 0.0;
    for msg in &ctx.all_messages {
        if let pick_ai::types::Message::Assistant(a) = msg {
            total_input += a.usage.input;
            total_output += a.usage.output;
            total_cache_read += a.usage.cache_read;
            total_cache_write += a.usage.cache_write;
            total_cost += a.usage.cost.total;
        }
    }
    let total_tokens = total_input + total_output + total_cache_read + total_cache_write;

    ctx.tui.chat.add_system_message(&format!("\x1b[1mSession Info\x1b[0m"));
    ctx.tui
        .chat
        .add_system_message(&format!("  \x1b[2mName:\x1b[0m    \x1b[1m{}\x1b[0m", name_display));
    ctx.tui
        .chat
        .add_system_message(&format!("  \x1b[2mID:\x1b[0m      {}", id));
    ctx.tui
        .chat
        .add_system_message(&format!("  \x1b[2mModel:\x1b[0m   {} ({})", ctx.model_id, ctx.provider));
    ctx.tui.chat.add_system_message(&format!("\x1b[1mMessages\x1b[0m"));
    ctx.tui
        .chat
        .add_system_message(&format!("  \x1b[2mUser:\x1b[0m      {}", user_msgs));
    ctx.tui
        .chat
        .add_system_message(&format!("  \x1b[2mAssistant:\x1b[0m {}", assistant_msgs));
    ctx.tui
        .chat
        .add_system_message(&format!("  \x1b[2mTool:\x1b[0m       {}", tool_msgs));
    ctx.tui
        .chat
        .add_system_message(&format!("  \x1b[2mTotal:\x1b[0m      {}", msg_count));
    ctx.tui.chat.add_system_message(&format!("\x1b[1mTokens\x1b[0m"));
    ctx.tui
        .chat
        .add_system_message(&format!("  \x1b[2mInput:\x1b[0m       {}", total_input));
    ctx.tui
        .chat
        .add_system_message(&format!("  \x1b[2mOutput:\x1b[0m      {}", total_output));
    if total_cache_read > 0 {
        ctx.tui
            .chat
            .add_system_message(&format!("  \x1b[2mCache Read:\x1b[0m  {}", total_cache_read));
    }
    if total_cache_write > 0 {
        ctx.tui
            .chat
            .add_system_message(&format!("  \x1b[2mCache Write:\x1b[0m {}", total_cache_write));
    }
    ctx.tui
        .chat
        .add_system_message(&format!("  \x1b[2mTotal:\x1b[0m       {}", total_tokens));
    if total_cost > 0.0 {
        ctx.tui.chat.add_system_message(&format!("\x1b[1mCost\x1b[0m"));
        ctx.tui
            .chat
            .add_system_message(&format!("  \x1b[2mTotal:\x1b[0m       ${:.4}", total_cost));
    }
}

/// Handle /name slash command
pub(crate) async fn handle_session_name(ctx: &mut TuiContext, name: &str) {
    if name.is_empty() {
        let current = ctx.tui.session_name.as_deref();
        if let Some(name) = current {
            ctx.tui
                .chat
                .add_system_message(&format!("Current session name: \x1b[1m{}\x1b[0m", name));
        } else if let Some(persisted) = ctx.session_manager.get_session_name() {
            ctx.tui
                .chat
                .add_system_message(&format!("Session name: \x1b[1m{}\x1b[0m", persisted));
        } else {
            ctx.tui.chat.add_system_message("Usage: /name <session name>");
        }
    } else {
        ctx.tui.set_session_name(name.to_string());
        if let Err(e) = ctx.session_manager.append_session_info(name).await {
            ctx.tui
                .show_error(&format!("Failed to persist session name: {}", e));
        }
        ctx.tui
            .chat
            .add_system_message(&format!("Session name set: \x1b[1m{}\x1b[0m", name));
    }
}

/// Handle /plan or /plan_enter slash command
pub(crate) async fn handle_plan(ctx: &mut TuiContext) {
    let was_plan = ctx.agent_mode == crate::core::agent_mode::AgentMode::Plan;
    if !was_plan {
        let change_entry = SessionEntry {
            id: uuid::Uuid::now_v7().to_string(),
            parent_id: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
            kind: pick_agent::session::SessionEntryKind::AgentModeChange(
                pick_agent::session::AgentModeChangeEntry {
                    from: ctx.agent_mode.to_string(),
                    to: crate::core::agent_mode::AgentMode::Plan.to_string(),
                },
            ),
        };
        if let Err(e) = ctx.session_manager.append(change_entry).await {
            ctx.tui
                .show_error(&format!("Failed to persist mode change: {}", e));
        }
        ctx.agent_mode = crate::core::agent_mode::AgentMode::Plan;
        ctx.tools = init::refilter_tools(&ctx.all_tools, &ctx.agent_mode, &ctx.session_manager);
        ctx.system_prompt = init::rebuild_system_prompt(
            &ctx.tools,
            &ctx.resource_loader,
            &ctx.cwd,
            &ctx.provider,
            &ctx.model_id,
            ctx.args.system_prompt.as_deref(),
            &ctx.args.append_system_prompt,
            Some(&ctx.agent_mode),
        );
        ctx.tui.agent_mode = ctx.agent_mode.to_string();
        ctx.tui
            .chat
            .add_system_message("\x1b[36mSwitched to PLAN mode (read-only, no edits)\x1b[0m");
    }
}

/// Handle /build or /plan_exit slash command
pub(crate) async fn handle_build(ctx: &mut TuiContext) {
    let from_plan = ctx.agent_mode == crate::core::agent_mode::AgentMode::Plan;
    if ctx.agent_mode != crate::core::agent_mode::AgentMode::Build {
        let change_entry = SessionEntry {
            id: uuid::Uuid::now_v7().to_string(),
            parent_id: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
            kind: pick_agent::session::SessionEntryKind::AgentModeChange(
                pick_agent::session::AgentModeChangeEntry {
                    from: ctx.agent_mode.to_string(),
                    to: crate::core::agent_mode::AgentMode::Build.to_string(),
                },
            ),
        };
        if let Err(e) = ctx.session_manager.append(change_entry).await {
            ctx.tui
                .show_error(&format!("Failed to persist mode change: {}", e));
        }
        ctx.agent_mode = crate::core::agent_mode::AgentMode::Build;
        ctx.tools = init::refilter_tools(&ctx.all_tools, &ctx.agent_mode, &ctx.session_manager);
        ctx.system_prompt = init::rebuild_system_prompt(
            &ctx.tools,
            &ctx.resource_loader,
            &ctx.cwd,
            &ctx.provider,
            &ctx.model_id,
            ctx.args.system_prompt.as_deref(),
            &ctx.args.append_system_prompt,
            Some(&ctx.agent_mode),
        );
        ctx.tui.agent_mode = ctx.agent_mode.to_string();
        let msg = if from_plan {
            crate::core::agent_mode::AgentMode::build_switch_prompt()
        } else {
            "Switched to BUILD mode"
        };
        ctx.tui
            .chat
            .add_system_message(&format!("\x1b[32m{}\x1b[0m", msg));
    }
}

/// Handle /changelog slash command
pub(crate) fn handle_changelog(ctx: &mut TuiContext) {
    use crate::utils::changelog::{get_changelog_path, parse_changelog};
    let changelog_path = get_changelog_path();
    if changelog_path.exists() {
        let entries = parse_changelog(&changelog_path);
        if entries.is_empty() {
            ctx.tui
                .chat
                .add_system_message("No changelog entries found.");
        } else {
            ctx.tui.chat.add_system_message("\x1b[1mChangelog\x1b[0m");
            let display = entries.iter().rev().take(5);
            for entry in display {
                let version = format!("v{}.{}.{}", entry.major, entry.minor, entry.patch);
                ctx.tui
                    .chat
                    .add_system_message(&format!("\x1b[1m{}\x1b[0m", version));
                let preview: Vec<&str> = entry.content.lines().take(5).collect();
                for line in &preview {
                    ctx.tui
                        .chat
                        .add_system_message(&format!("\x1b[2m  {}\x1b[0m", line));
                }
                if entry.content.lines().count() > 5 {
                    ctx.tui.chat.add_system_message("\x1b[2m  ...\x1b[0m");
                }
            }
        }
    } else {
        ctx.tui.chat.add_system_message("Changelog not found.");
    }
}

/// Handle /hotkeys slash command
pub(crate) fn handle_hotkeys(ctx: &mut TuiContext) {
    ctx.tui.chat.add_system_message("\x1b[1mKeyboard Shortcuts\x1b[0m");
    ctx.tui.chat.add_system_message("");
    ctx.tui.chat.add_system_message("\x1b[1mNavigation\x1b[0m");
    ctx.tui.chat.add_system_message("  \x1b[2m↑/↓\x1b[0m            Move cursor / browse history");
    ctx.tui.chat.add_system_message("  \x1b[2m←/→\x1b[0m            Move cursor left/right");
    ctx.tui.chat.add_system_message("  \x1b[2mCtrl+←/→\x1b[0m       Move by word");
    ctx.tui.chat.add_system_message("  \x1b[2mHome/End\x1b[0m        Start/end of line");
    ctx.tui.chat.add_system_message("  \x1b[2mPageUp/Down\x1b[0m    Scroll chat");
    ctx.tui.chat.add_system_message("");
    ctx.tui.chat.add_system_message("\x1b[1mEditing\x1b[0m");
    ctx.tui.chat.add_system_message("  \x1b[2mEnter\x1b[0m            Send message");
    ctx.tui.chat.add_system_message("  \x1b[2mShift+Enter\x1b[0m     New line");
    ctx.tui.chat.add_system_message("  \x1b[2mTab\x1b[0m              Autocomplete / next suggestion");
    ctx.tui.chat.add_system_message("  \x1b[2mEsc\x1b[0m              Cancel autocomplete");
    ctx.tui.chat.add_system_message("  \x1b[2mCtrl+C/D\x1b[0m        Quit");
    ctx.tui.chat.add_system_message("  \x1b[2mCtrl+U\x1b[0m          Delete to line start");
    ctx.tui.chat.add_system_message("  \x1b[2mCtrl+K\x1b[0m          Delete to line end");
    ctx.tui.chat.add_system_message("  \x1b[2mCtrl+W\x1b[0m          Delete word backward");
    ctx.tui.chat.add_system_message("  \x1b[2mCtrl+Z\x1b[0m          Undo");
    ctx.tui.chat.add_system_message("");
    ctx.tui.chat.add_system_message("\x1b[1mCommands\x1b[0m");
    ctx.tui.chat.add_system_message("  \x1b[2m/command\x1b[0m        Slash commands");
    ctx.tui.chat.add_system_message("  \x1b[2m!command\x1b[0m        Run bash command");
    ctx.tui.chat.add_system_message("  \x1b[2m!!command\x1b[0m       Run bash (excluded from context)");
    ctx.tui.chat.add_system_message("  \x1b[2m/help\x1b[0m           Show all commands");
}

/// Handle /quit slash command
pub(crate) async fn handle_quit(ctx: &mut TuiContext) {
    ctx.tui.chat.add_system_message("Goodbye!");
    ctx.mcp_cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
    ctx.mcp_manager.shutdown().await;
}
