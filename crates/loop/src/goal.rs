//! Goal prompt builder — constructs the prompt text for goal-mode loops.

use crate::types::LoopJob;

/// Build a goal-mode prompt for the agent.
pub fn build_goal_prompt(job: &LoopJob) -> String {
    let criteria: String = job
        .goal_acceptance
        .iter()
        .map(|c| format!("- {}", c))
        .collect::<Vec<_>>()
        .join("\n");

    let checks: String = job
        .goal_checks
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{}. {}", i + 1, c))
        .collect::<Vec<_>>()
        .join("\n");

    let progress: String = job
        .goal_progress
        .iter()
        .rev()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"EXPERIMENTAL LOOP GOAL MODE ITERATION.

You are pursuing an experimental persistent goal for this Pick session.

OBJECTIVE: {objective}

ACCEPTANCE CRITERIA:
{criteria}

VERIFICATION CHECKS:
{checks}{verify_section}{progress_section}

RULES:
1. Work on the next smallest useful step toward the goal.
2. Prefer direct code changes, tests, typechecks, builds, and evidence over discussion.
3. Do not claim the goal is complete unless the acceptance criteria are satisfied.
4. When complete → call `opencode_loop_goal_complete` with a summary and evidence.
5. If truly blocked → call `opencode_loop_goal_blocked` with the reason and what is needed.
6. If meaningful progress is made → call `opencode_loop_goal_progress` with a summary and next step.
7. Do not ask questions unless blocked; make reasonable assumptions.
8. Follow safety rules — do not run dangerous shell commands without confirmation.

AVAILABLE GOAL TOOLS:
- opencode_loop_goal_complete(summary, evidence)
- opencode_loop_goal_blocked(reason, needed)
- opencode_loop_goal_progress(summary, next)"#,
        objective = job.action,
        criteria = if criteria.is_empty() {
            "  (none specified)".to_string()
        } else {
            criteria
        },
        checks = if checks.is_empty() {
            "  (none specified)".to_string()
        } else {
            checks
        },
        verify_section = if let Some(cmd) = &job.verify_command {
            format!("\n\nVERIFICATION COMMAND: {}", cmd)
        } else {
            String::new()
        },
        progress_section = if progress.is_empty() {
            String::new()
        } else {
            format!("\n\nRECENT PROGRESS:\n{}", progress)
        },
    )
}

/// Decorate a prompt with additional instructions.
pub fn decorate_prompt(job: &LoopJob) -> String {
    let mut parts = vec![job.action.clone()];

    if let Some(ref fail) = job.last_verify_failure {
        parts.push(format!("\n\n[Previous verify failed: {}]", fail));
    }
    if job.ask_never {
        parts.push("\n\n[Do not ask questions — make reasonable assumptions.]".into());
    }
    if job.safe {
        parts.push("\n\n[Safe mode: do not run dangerous shell commands.]".into());
    }
    if let Some(batch) = job.batch {
        parts.push(format!("\n\n[Process at most {} items per turn.]", batch));
    }
    if job.quiet {
        parts.push("\n\n[Keep responses short and concise.]".into());
    }

    parts.join("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LoopJob;

    #[test]
    fn test_build_goal_prompt_contains_objective() {
        let job = LoopJob::new_goal(
            "g1".into(),
            "Refactor the database layer".into(),
            vec!["All tests pass".into()],
            vec!["cargo test".into()],
            0,
        );
        let prompt = build_goal_prompt(&job);
        assert!(prompt.contains("Refactor the database layer"));
        assert!(prompt.contains("All tests pass"));
        assert!(prompt.contains("cargo test"));
        assert!(prompt.contains("opencode_loop_goal_complete"));
    }

    #[test]
    fn test_decorate_prompt_quiet() {
        let mut job = LoopJob::new_prompt("t".into(), "".into(), "do stuff".into(), 0, true);
        job.quiet = true;
        let prompt = decorate_prompt(&job);
        assert!(prompt.contains("Keep responses short"));
    }

    #[test]
    fn test_decorate_prompt_verify_failure() {
        let mut job = LoopJob::new_prompt("t".into(), "".into(), "do stuff".into(), 0, true);
        job.last_verify_failure = Some("build failed".into());
        let prompt = decorate_prompt(&job);
        assert!(prompt.contains("build failed"));
    }
}
