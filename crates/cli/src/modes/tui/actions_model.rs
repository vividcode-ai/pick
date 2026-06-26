use pick_agent::session::{AgentModeChangeEntry, SessionEntry, SessionEntryKind};
use pick_ai::types::{Message, UserMessage};
use pick_tui::components::select::{SelectItem, SelectList};
use std::sync::atomic::Ordering;

use super::context::TuiContext;
use super::init;

/// Handle CycleModel action: cycle to next model in list
pub(crate) async fn handle_cycle_model(ctx: &mut TuiContext) {
    let all_models = pick_ai::models::get_models(&ctx.provider);
    let cycle_list: Vec<_> = if ctx.scoped_models.is_empty() {
        all_models
            .iter()
            .map(|m| (m.id.clone(), m.provider.as_str().to_string()))
            .collect()
    } else {
        ctx.scoped_models
            .iter()
            .filter_map(|sm| {
                all_models
                    .iter()
                    .find(|m| m.id == *sm)
                    .map(|m| (m.id.clone(), m.provider.as_str().to_string()))
            })
            .collect()
    };

    if cycle_list.is_empty() {
        ctx.tui
            .chat
            .add_system_message("No models available for cycling.");
        ctx.tui.finalize_turn();
        return;
    }

    let current_pos = cycle_list.iter().position(|(id, _)| *id == ctx.model_id);
    let next_idx = current_pos.map(|p| (p + 1) % cycle_list.len()).unwrap_or(0);
    let (next_model_id, next_provider) = &cycle_list[next_idx];

    if *next_model_id != ctx.model_id {
        switch_to_model(ctx, next_model_id, Some(next_provider)).await;
    }
    ctx.tui.finalize_turn();
}

/// Handle CycleModelBackward action: cycle to previous model
pub(crate) async fn handle_cycle_model_backward(ctx: &mut TuiContext) {
    let all_models = pick_ai::models::get_models(&ctx.provider);
    let cycle_list: Vec<_> = if ctx.scoped_models.is_empty() {
        all_models
            .iter()
            .map(|m| (m.id.clone(), m.provider.as_str().to_string()))
            .collect()
    } else {
        ctx.scoped_models
            .iter()
            .filter_map(|sm| {
                all_models
                    .iter()
                    .find(|m| m.id == *sm)
                    .map(|m| (m.id.clone(), m.provider.as_str().to_string()))
            })
            .collect()
    };

    if cycle_list.is_empty() {
        ctx.tui
            .chat
            .add_system_message("No models available for cycling.");
        ctx.tui.finalize_turn();
        return;
    }

    let current_pos = cycle_list.iter().position(|(id, _)| *id == ctx.model_id);
    let prev_idx = current_pos
        .map(|p| if p == 0 { cycle_list.len() - 1 } else { p - 1 })
        .unwrap_or(cycle_list.len() - 1);
    let (next_model_id, next_provider) = &cycle_list[prev_idx];

    if *next_model_id != ctx.model_id {
        switch_to_model(ctx, next_model_id, Some(next_provider)).await;
    }
    ctx.tui.finalize_turn();
}

/// Handle SelectModel action: show model selection list
pub(crate) fn handle_select_model(ctx: &mut TuiContext) {
    ctx.pending_command = Some("model".to_string());
    let models = pick_ai::models::get_models(&ctx.provider);
    let items: Vec<SelectItem> = if models.is_empty() {
        vec![
            SelectItem::new(ctx.model_id.clone(), ctx.model_id.clone()).with_description("Current"),
        ]
    } else {
        models
            .iter()
            .map(|m| SelectItem::new(m.id.clone(), m.id.clone()).with_description(m.name.clone()))
            .collect()
    };
    let select = SelectList::new("Models", items);
    ctx.tui.start_selection(select);
    ctx.tui.finalize_turn();
}

/// Handle CycleThinking action
pub(crate) fn handle_cycle_thinking(ctx: &mut TuiContext) {
    use pick_ai::models::get_supported_thinking_levels;
    use pick_ai::types::ThinkingLevel as AiThinkingLevel;

    // Get model-supported levels for cycling, falling back to common defaults
    let ai_model = pick_ai::models::get_model(&ctx.provider, &ctx.model_id);
    let ai_supported = ai_model
        .as_ref()
        .map(|m| get_supported_thinking_levels(m))
        .unwrap_or_default();
    let levels: Vec<pick_agent::core::state::ThinkingLevel> = if ai_supported.is_empty() {
        vec![
            pick_agent::core::state::ThinkingLevel::Off,
            pick_agent::core::state::ThinkingLevel::Low,
            pick_agent::core::state::ThinkingLevel::Medium,
            pick_agent::core::state::ThinkingLevel::High,
        ]
    } else {
        ai_supported
            .iter()
            .map(|l| match l {
                AiThinkingLevel::Off => pick_agent::core::state::ThinkingLevel::Off,
                AiThinkingLevel::Minimal => pick_agent::core::state::ThinkingLevel::Minimal,
                AiThinkingLevel::Low => pick_agent::core::state::ThinkingLevel::Low,
                AiThinkingLevel::Medium => pick_agent::core::state::ThinkingLevel::Medium,
                AiThinkingLevel::High => pick_agent::core::state::ThinkingLevel::High,
                AiThinkingLevel::XHigh => pick_agent::core::state::ThinkingLevel::XHigh,
            })
            .collect()
    };

    let current_idx = levels
        .iter()
        .position(|l| *l == ctx.thinking_level)
        .unwrap_or(0);
    let next_level = levels[(current_idx + 1) % levels.len()];
    ctx.thinking_level = next_level;
    let level_str = format!("{:?}", ctx.thinking_level).to_lowercase();
    ctx.tui.thinking_level = level_str.clone();
    ctx.tui.chat.add_system_message(&format!(
        "Thinking level set to \x1b[1m{}\x1b[0m.",
        level_str
    ));
    ctx.tui.finalize_turn();
}

/// Handle CycleMode action: toggle Plan ↔ Build
pub(crate) async fn handle_cycle_mode(ctx: &mut TuiContext) {
    let new_mode = if ctx.agent_mode == crate::core::agent_mode::AgentMode::Plan {
        crate::core::agent_mode::AgentMode::Build
    } else {
        crate::core::agent_mode::AgentMode::Plan
    };

    let from_plan = ctx.agent_mode == crate::core::agent_mode::AgentMode::Plan;
    ctx.agent_mode = new_mode;
    ctx.tui.agent_mode = ctx.agent_mode.to_string();

    // Persist mode change
    let change_entry = SessionEntry {
        id: uuid::Uuid::now_v7().to_string(),
        parent_id: None,
        timestamp: chrono::Utc::now().timestamp_millis(),
        kind: SessionEntryKind::AgentModeChange(AgentModeChangeEntry {
            from: if from_plan { "plan" } else { "build" }.to_string(),
            to: ctx.agent_mode.to_string(),
        }),
    };
    if let Err(e) = ctx.session_manager.append(change_entry).await {
        ctx.tui
            .show_error(&format!("Failed to persist mode change: {}", e));
    }

    // Rebuild tools and prompt
    ctx.tools = init::refilter_tools(
        &ctx.all_tools,
        &ctx.agent_mode,
        &ctx.session_manager,
        &ctx.mcp_manager,
        ctx.mcp_enabled.load(Ordering::Relaxed),
    )
    .await;
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

    // Inject mode switch into conversation history so LLM is aware of mode change
    if from_plan {
        ctx.all_messages.push(Message::User(UserMessage::text(
            crate::core::agent_mode::AgentMode::build_switch_prompt(),
        )));
    }

    ctx.tui.finalize_turn();
}

/// Switch to a different model/provider and update state
async fn switch_to_model(ctx: &mut TuiContext, new_model_id: &str, new_provider: Option<&str>) {
    let resolved_provider = match new_provider {
        Some(p) if p != ctx.provider => p.to_string(),
        _ => ctx.provider.clone(),
    };

    let (new_model, _) = init::update_model(&resolved_provider, new_model_id);
    ctx.model = new_model;
    ctx.model_id = new_model_id.to_string();
    ctx.provider = resolved_provider;
    init::save_default_model(&ctx.provider, &ctx.model_id);

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
    init::update_api_key(&ctx.auth, &ctx.provider).await;

    ctx.tui.model_id = ctx.model_id.clone();
    ctx.tui.provider = ctx.provider.clone();
    ctx.tui.chat.add_system_message(&format!(
        "Switched to model: \x1b[1m{} ({})\x1b[0m",
        ctx.model_id, ctx.provider
    ));
}

/// Refresh tools and system prompt after mode/MCP change
pub(crate) async fn refresh_tools_and_prompt(ctx: &mut TuiContext) {
    ctx.tools = init::refilter_tools(
        &ctx.all_tools,
        &ctx.agent_mode,
        &ctx.session_manager,
        &ctx.mcp_manager,
        ctx.mcp_enabled.load(Ordering::Relaxed),
    )
    .await;
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
}
