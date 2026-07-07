//! Tool registry for managing available tools

use std::sync::Arc;

use pick_ai::types::{ContentBlock, JsonSchema};

use crate::core::state::{AgentTool, AgentToolResult, ToolExecutionMode};
use crate::extensions::runner::ExtensionRunner;
use crate::extensions::types::ToolParameter;
use crate::session::goal::GoalManager;

/// Registry of available tools
#[derive(Default)]
pub struct ToolRegistry {
    tools: Vec<AgentTool>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: AgentTool) {
        self.tools.push(tool);
    }

    pub fn get_all(&self) -> &[AgentTool] {
        &self.tools
    }

    pub fn get(&self, name: &str) -> Option<&AgentTool> {
        self.tools.iter().find(|t| t.name == name)
    }

    pub fn into_vec(self) -> Vec<AgentTool> {
        self.tools
    }
}

/// Create the default set of coding tools
pub fn create_coding_tools() -> Vec<AgentTool> {
    create_coding_tools_with_mode(None)
}

/// Create coding tools with optional agent mode for subagent inheritance
pub fn create_coding_tools_with_mode(agent_mode: Option<String>) -> Vec<AgentTool> {
    vec![
        super::read::create_read_tool(),
        super::write::create_write_tool(),
        super::edit::create_edit_tool(),
        super::bash::create_bash_tool(),
        super::grep::create_grep_tool(),
        super::find::create_find_tool(),
        super::ls::create_ls_tool(),
        super::subagent::create_subagent_tool_with_mode(agent_mode),
        super::webfetch::create_webfetch_tool(),
        super::todo_plan::create_todo_plan_tool(),
        super::question::create_question_tool(),
        super::goal::create_goal_tool_stub(),
    ]
}

/// Create coding tools with optional agent mode and a real GoalManager.
/// Goal tools are wired to the GoalManager instead of returning stubs.
pub fn create_coding_tools_with_goal_manager(
    agent_mode: Option<String>,
    goal_manager: Arc<GoalManager>,
) -> Vec<AgentTool> {
    let mut tools: Vec<AgentTool> = vec![
        super::read::create_read_tool(),
        super::write::create_write_tool(),
        super::edit::create_edit_tool(),
        super::bash::create_bash_tool(),
        super::grep::create_grep_tool(),
        super::find::create_find_tool(),
        super::ls::create_ls_tool(),
        super::subagent::create_subagent_tool_with_mode(agent_mode),
        super::webfetch::create_webfetch_tool(),
        super::todo_plan::create_todo_plan_tool(),
        super::question::create_question_tool(),
    ];
    // Goal tool with real GoalManager
    tools.push(super::goal::create_goal_tool(goal_manager.clone()));
    tools
}

/// Create AgentTool instances from extension-registered tools.
/// Each tool's execute callback dispatches through the ExtensionRunner.
pub fn create_extension_tools(runner: Arc<ExtensionRunner>) -> Vec<AgentTool> {
    let registered = runner.get_all_registered_tools();
    let mut tools = Vec::with_capacity(registered.len());

    for rt in registered {
        let tool_name = rt.definition.name.clone();
        let runner_captured = runner.clone();

        let agent_tool = AgentTool {
            name: rt.definition.name.clone(),
            description: rt.definition.description.clone(),
            prompt_snippet: rt.definition.prompt_snippet.clone(),
            prompt_guidelines: rt.definition.prompt_guidelines.clone().unwrap_or_default(),
            usage_example: rt.definition.usage_example.clone(),
            label: rt.definition.label.clone(),
            parameters: tool_params_to_json_schema(&rt.definition.parameters),
            execute: std::sync::Arc::new(move |tool_call_id, args, _ctx| {
                let r = runner_captured.clone();
                let name = tool_name.clone();
                Box::pin(async move {
                    match r.execute_extension_tool(&tool_call_id, &name, args) {
                        Some(result) => {
                            let (content, is_error) = parse_tool_result(&result);
                            Ok(AgentToolResult {
                                content,
                                is_error,
                                terminate: false,
                            })
                        }
                        None => Err(format!(
                            "Extension tool '{}' did not produce a result",
                            name
                        )),
                    }
                })
            }),
            execution_mode: ToolExecutionMode::Sequential,
        };

        tools.push(agent_tool);
    }

    tools
}

/// Convert extension ToolParameter list to JsonSchema (object with properties).
fn tool_params_to_json_schema(params: &[ToolParameter]) -> JsonSchema {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for p in params {
        let prop = if p.schema.is_null() || p.schema.as_object().is_none_or(|o| o.is_empty()) {
            serde_json::json!({
                "type": "string",
                "description": p.description
            })
        } else {
            let mut s = p.schema.clone();
            if let Some(obj) = s.as_object_mut()
                && !obj.contains_key("description")
            {
                obj.insert(
                    "description".to_string(),
                    serde_json::Value::String(p.description.clone()),
                );
            }
            s
        };

        properties.insert(p.name.clone(), prop);
        if p.required {
            required.push(p.name.clone());
        }
    }

    JsonSchema {
        schema_type: "object".to_string(),
        properties: Some(properties),
        required: if required.is_empty() {
            None
        } else {
            Some(required)
        },
        description: None,
        items: None,
        additional_properties: Some(false),
    }
}

/// Filter out the goal tool when no active goal exists.
/// The goal tool should only be visible to the LLM when a goal has been set via /goal.
pub fn filter_goal_tools(
    tools: Vec<AgentTool>,
    goal_manager: std::sync::Arc<crate::session::goal::GoalManager>,
) -> Vec<AgentTool> {
    if goal_manager.get().is_some() {
        return tools;
    }
    tools.into_iter().filter(|t| t.name != "goal").collect()
}

/// Extract content blocks and is_error flag from an extension tool result.
fn parse_tool_result(result: &serde_json::Value) -> (Vec<ContentBlock>, bool) {
    let content = result
        .get("content")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    if let Some(text) = v.as_str() {
                        Some(ContentBlock::text(text))
                    } else {
                        v.get("text")
                            .and_then(|t| t.as_str())
                            .map(ContentBlock::text)
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![ContentBlock::text(result.to_string())]);

    let is_error = result
        .get("is_error")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    (content, is_error)
}
