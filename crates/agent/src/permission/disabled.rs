use std::collections::HashSet;

use super::Ruleset;

pub fn disabled_tools(tools: &[String], rulesets: &[&Ruleset]) -> HashSet<String> {
    let mut disabled = HashSet::new();

    for tool_name in tools {
        if super::evaluate::is_tool_disabled(tool_name, rulesets) {
            disabled.insert(tool_name.clone());
        }
    }

    disabled
}

pub fn filter_tools(
    tools: Vec<crate::core::state::AgentTool>,
    rulesets: &[&Ruleset],
) -> Vec<crate::core::state::AgentTool> {
    let tool_names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
    let disabled = disabled_tools(&tool_names, rulesets);

    tools
        .into_iter()
        .filter(|t| !disabled.contains(&t.name))
        .collect()
}
