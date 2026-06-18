use std::sync::Arc;

use pick_agent::core::state::{AgentTool, AgentToolResult, ToolContext, ToolExecutionMode};
use pick_ai::types::ContentBlock;
use pick_ai::types::tool::JsonSchema;

use crate::core::agent_mode::AgentMode;

pub fn create_plan_enter_tool(on_switch: Arc<dyn Fn(AgentMode) + Send + Sync>) -> AgentTool {
    AgentTool {
        name: "plan_enter".to_string(),
        description: AgentMode::plan_enter_description().to_string(),
        prompt_snippet: Some("Suggest switching to plan mode for complex tasks".to_string()),
        prompt_guidelines: vec![],
        label: "plan_enter".to_string(),
        parameters: JsonSchema {
            schema_type: "object".to_string(),
            properties: None,
            required: None,
            description: None,
            items: None,
            additional_properties: None,
        },
        execute: Arc::new(
            move |_id: String, _args: serde_json::Value, _ctx: ToolContext| {
                let on_switch = on_switch.clone();
                Box::pin(async move {
                    on_switch(AgentMode::Plan);
                    Ok(AgentToolResult {
                        content: vec![ContentBlock::text(
                            "Switched to plan mode. You are now in read-only mode.",
                        )],
                        is_error: false,
                        terminate: true,
                    })
                })
            },
        ),
        execution_mode: ToolExecutionMode::Sequential,
    }
}

pub fn create_plan_exit_tool(on_switch: Arc<dyn Fn(AgentMode) + Send + Sync>) -> AgentTool {
    AgentTool {
        name: "plan_exit".to_string(),
        description: AgentMode::plan_exit_description().to_string(),
        prompt_snippet: Some("Exit plan mode and start implementing".to_string()),
        prompt_guidelines: vec![],
        label: "plan_exit".to_string(),
        parameters: JsonSchema {
            schema_type: "object".to_string(),
            properties: None,
            required: None,
            description: None,
            items: None,
            additional_properties: None,
        },
        execute: Arc::new(
            move |_id: String, _args: serde_json::Value, _ctx: ToolContext| {
                let on_switch = on_switch.clone();
                Box::pin(async move {
                    on_switch(AgentMode::Build);
                    Ok(AgentToolResult {
                        content: vec![ContentBlock::text(
                            "Switched to build mode. You may now edit files and run commands.",
                        )],
                        is_error: false,
                        terminate: true,
                    })
                })
            },
        ),
        execution_mode: ToolExecutionMode::Sequential,
    }
}
