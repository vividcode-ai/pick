use crate::session::{GoalEntry, GoalManager};

fn render_goal_template(template: &str, vars: &[(&str, &str)]) -> String {
    use std::collections::HashMap;
    let vars: HashMap<&str, &str> = vars.iter().copied().collect();
    let mut result = template.to_string();
    for (key, value) in &vars {
        let padded = format!("{{{{ {} }}}}", key);
        result = result.replace(&padded, value);
        let tight = format!("{{{{{}}}}}", key);
        result = result.replace(&tight, value);
    }
    result
}

fn escape_xml_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn criterion_display(criterion: &str) -> String {
    if criterion.is_empty() {
        "none".to_string()
    } else {
        criterion.to_string()
    }
}

fn token_budget_str(budget: Option<i64>) -> String {
    budget
        .map(|b| b.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn remaining_tokens_str(gm: &GoalManager) -> String {
    gm.remaining_tokens()
        .map(|r| r.to_string())
        .unwrap_or_else(|| "unbounded".to_string())
}

/// Render the steering_active.md template with goal context.
pub fn render_steering_active(goal: &GoalEntry, gm: &GoalManager) -> String {
    let objective = escape_xml_text(&goal.objective);
    let criterion = escape_xml_text(&goal.completion_criterion);
    render_goal_template(
        include_str!("goals/steering_active.md"),
        &[
            ("objective", &objective),
            ("completion_criterion", &criterion_display(&criterion)),
            ("tokens_used", &goal.tokens_used.to_string()),
            ("token_budget", &token_budget_str(goal.token_budget)),
            ("remaining_tokens", &remaining_tokens_str(gm)),
            ("time_used_seconds", &goal.time_used_seconds.to_string()),
        ],
    )
}

/// Render the follow_up_continuation.md template for auto-continue.
pub fn render_follow_up_continuation(goal: &GoalEntry, gm: &GoalManager) -> String {
    let objective = escape_xml_text(&goal.objective);
    let criterion = escape_xml_text(&goal.completion_criterion);
    render_goal_template(
        include_str!("goals/follow_up_continuation.md"),
        &[
            ("objective", &objective),
            ("completion_criterion", &criterion_display(&criterion)),
            ("tokens_used", &goal.tokens_used.to_string()),
            ("token_budget", &token_budget_str(goal.token_budget)),
            ("remaining_tokens", &remaining_tokens_str(gm)),
        ],
    )
}

/// Render the objective_updated.md template.
pub fn render_objective_updated(goal: &GoalEntry, gm: &GoalManager) -> String {
    let objective = escape_xml_text(&goal.objective);
    render_goal_template(
        include_str!("goals/objective_updated.md"),
        &[
            ("objective", &objective),
            ("tokens_used", &goal.tokens_used.to_string()),
            ("token_budget", &token_budget_str(goal.token_budget)),
            ("remaining_tokens", &remaining_tokens_str(gm)),
        ],
    )
}

/// Render the steering_budget_limit.md template.
pub fn render_steering_budget_limit(goal: &GoalEntry) -> String {
    let objective = escape_xml_text(&goal.objective);
    render_goal_template(
        include_str!("goals/steering_budget_limit.md"),
        &[
            ("objective", &objective),
            ("tokens_used", &goal.tokens_used.to_string()),
            ("token_budget", &token_budget_str(goal.token_budget)),
            ("time_used_seconds", &goal.time_used_seconds.to_string()),
        ],
    )
}
