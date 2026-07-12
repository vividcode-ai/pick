//! AgentLoopConfig hook factories.
//!
//! These functions create hooks that wire a LoopManager and LoopScheduler
//! into the agent loop. They use the **hook composition** pattern — wrapping
//! existing hooks so loop logic runs alongside existing goal/steering logic.
//!
//! Each factory takes the existing hook (if any) and returns a new hook that
//! calls the old one internally.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use pick_agent::core::agent_loop::AgentRunResult;
use pick_ai::types::{AssistantMessage, Message, UserMessage};
use tokio::sync::RwLock;

use crate::goal::{build_goal_prompt, decorate_prompt};
use crate::manager::LoopManager;
use crate::scheduler::LoopScheduler;
use crate::types::LoopJob;

// ── Hook factories ─────────────────────────────────────────────────────────

/// Build a steering messages hook that injects loop prompt messages.
///
/// Works by checking the LoopManager for due jobs and converting them to
/// user messages that the agent loop picks up before each turn.
///
/// Composes with an existing steering hook (e.g., for goals).
pub fn build_steering_hook(
    inner_hook: Option<Arc<dyn Fn() -> Vec<Message> + Send + Sync>>,
    loop_manager: Arc<RwLock<LoopManager>>,
) -> Arc<dyn Fn() -> Vec<Message> + Send + Sync> {
    Arc::new(move || {
        let mut msgs = Vec::new();

        // 1. Run existing hook first (goal / steer queue)
        if let Some(ref inner) = inner_hook {
            msgs.extend(inner());
        }

        // 2. Check for loop messages — we look for running jobs
        //    that need a prompt injected.
        //    (Due job triggering is done by the scheduler, not here.)
        let mgr = loop_manager.try_read();
        if let Ok(mgr) = mgr {
            for job in mgr.list() {
                // If a running goal job exists, inject goal follow-up
                if job.is_goal() && job.status == crate::types::LoopJobStatus::Running {
                    let prompt = build_goal_prompt(job);
                    msgs.push(Message::User(UserMessage::text(prompt)));
                }
            }
        }

        msgs
    })
}

/// Build a follow-up messages hook for immediate (interval=0) loops.
///
/// After a turn completes, if a loop job is configured for immediate re-run,
/// re-enqueue it so the outer loop picks it up.
///
/// NOTE: interval_ms=0 follow-up re-injection has been removed because it
/// caused the inner agent loop to run forever (each turn re-injected the
/// same prompt). interval_ms > 0 jobs are re-triggered by the scheduler's
/// timer. interval_ms=0 jobs fire once on creation; manual re-triggering
/// via the API or a new session-idle event replaces follow-up injection.
pub fn build_follow_up_hook(
    inner_hook: Option<Arc<dyn Fn(&AgentRunResult) -> Vec<Message> + Send + Sync>>,
    _loop_manager: Arc<RwLock<LoopManager>>,
) -> Arc<dyn Fn(&AgentRunResult) -> Vec<Message> + Send + Sync> {
    Arc::new(move |result| {
        let mut msgs = Vec::new();

        // 1. Run existing hook
        if let Some(ref inner) = inner_hook {
            msgs.extend(inner(result));
        }

        msgs
    })
}

/// Build a should-stop hook that checks loop job limits.
pub fn build_should_stop_hook(
    inner_hook: Option<Arc<dyn Fn(&AssistantMessage) -> bool + Send + Sync>>,
    loop_manager: Arc<RwLock<LoopManager>>,
) -> Arc<dyn Fn(&AssistantMessage) -> bool + Send + Sync> {
    Arc::new(move |msg| {
        // 1. Check existing hook
        if let Some(ref inner) = inner_hook {
            if inner(msg) {
                return true;
            }
        }

        // 2. Check if any running loop job has exceeded limits
        if let Ok(mgr) = loop_manager.try_read() {
            let now = chrono::Utc::now().timestamp_millis();
            for job in mgr.list() {
                if job.status == crate::types::LoopJobStatus::Running {
                    if job.max_runtime_exceeded(now) {
                        return true;
                    }
                    if job.timeout_ms.map_or(false, |t| {
                        job.last_run_at
                            .map(|last| (now - last) >= t as i64)
                            .unwrap_or(false)
                    }) {
                        return true;
                    }
                }
            }
        }

        false
    })
}

/// Build an on_turn_complete hook that runs verify, postrun, checkpoint,
/// and reschedule logic for loop jobs.
///
/// `loop_scheduler` is optional — when `Some`, idle jobs are rescheduled
/// so their next trigger is `interval_ms` from the completion of this run
/// rather than from the original `schedule()` call.
pub fn build_turn_complete_hook(
    inner_hook: Option<
        Arc<dyn Fn(&[Message]) -> Pin<Box<dyn Send + Future<Output = ()>>> + Send + Sync>,
    >,
    loop_manager: Arc<RwLock<LoopManager>>,
    loop_scheduler: Option<Arc<LoopScheduler>>,
) -> Arc<dyn Fn(&[Message]) -> Pin<Box<dyn Send + Future<Output = ()>>> + Send + Sync> {
    Arc::new(move |messages| {
        let lm = loop_manager.clone();
        let ls = loop_scheduler.clone();
        let inner = inner_hook.clone();
        // Clone messages so they can be moved into the async block
        let msgs_clone: Vec<Message> = messages.to_vec();
        Box::pin(async move {
            // 1. Run existing hook first
            if let Some(ref inner) = inner {
                inner(&msgs_clone).await;
            }

            // 2. Find running loop jobs and finalize them
            let running_ids: Vec<String> = {
                if let Ok(mgr) = lm.try_read() {
                    mgr.list()
                        .iter()
                        .filter(|j| j.status == crate::types::LoopJobStatus::Running)
                        .map(|j| j.id.clone())
                        .collect()
                } else {
                    return;
                }
            };

            for id in running_ids {
                let reached_max = {
                    let mut mgr = lm.write().await;
                    let reached_max = mgr.record_run(&id);

                    // Run verify command (if configured)
                    let verify_cmd = mgr.get(&id).and_then(|j| j.verify_command.clone());
                    if let Some(ref cmd) = verify_cmd {
                        let output = tokio::process::Command::new("sh")
                            .arg("-c")
                            .arg(cmd)
                            .output()
                            .await;
                        match output {
                            Ok(out) if !out.status.success() => {
                                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                                mgr.record_failure(&id, Some(stderr));
                            }
                            Ok(_) => {
                                if let Some(job) = mgr.get_mut(&id) {
                                    job.failure_count = 0;
                                    job.last_verify_failure = None;
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to run verify for job {}: {}", id, e);
                            }
                        }
                    }

                    // Run postrun command (if configured)
                    let postrun_cmd = mgr.get(&id).and_then(|j| j.postrun_command.clone());
                    if let Some(ref cmd) = postrun_cmd {
                        let _ = tokio::process::Command::new("sh")
                            .arg("-c")
                            .arg(cmd)
                            .output()
                            .await;
                    }

                    if reached_max {
                        mgr.mark_done(&id);
                    } else {
                        mgr.mark_idle(&id);
                    }

                    let _ = mgr.save();
                    reached_max
                }; // <-- mgr write guard dropped HERE (before reschedule)

                // Reschedule the timer so the next fire is `interval_ms` from
                // now (not from the original schedule() call). This ensures
                // the interval is measured between agent run completions.
                if let Some(ref ls) = ls {
                    if !reached_max {
                        if let Some(job) = lm.read().await.get(&id).cloned() {
                            ls.schedule(&job).await;
                        }
                    }
                }
            }
        })
    })
}

/// Helper to build a user message from a loop job.
pub fn build_loop_message(job: &LoopJob) -> Message {
    let text = match job.kind.as_str() {
        "goal" | _ if job.is_goal() => build_goal_prompt(job),
        _ => decorate_prompt(job),
    };
    Message::User(UserMessage::text(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LoopJob;

    #[test]
    fn test_build_steering_hook_no_inner() {
        let mgr = Arc::new(RwLock::new(LoopManager::new("test.json".into())));
        let hook = build_steering_hook(None, mgr);
        let msgs = hook();
        // No running jobs → nothing injected
        assert!(msgs.is_empty());
    }

    #[test]
    fn test_build_steering_hook_with_inner() {
        let mgr = Arc::new(RwLock::new(LoopManager::new("test.json".into())));
        let inner: Option<Arc<dyn Fn() -> Vec<Message> + Send + Sync>> = Some(Arc::new(|| {
            vec![Message::User(UserMessage::text("from inner"))]
        }));
        let hook = build_steering_hook(inner, mgr);
        let msgs = hook();
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn test_build_loop_message_goal() {
        let job = LoopJob::new_goal(
            "g1".into(),
            "refactor".into(),
            vec!["tests pass".into()],
            vec![],
            0,
        );
        let msg = build_loop_message(&job);
        let text = match &msg {
            Message::User(u) => u
                .content
                .iter()
                .filter_map(|b| {
                    if let pick_ai::types::ContentBlock::Text(t) = b {
                        Some(t.text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(" "),
            _ => String::new(),
        };
        assert!(text.contains("EXPERIMENTAL LOOP GOAL MODE"));
        assert!(text.contains("refactor"));
    }

    #[test]
    fn test_build_loop_message_prompt() {
        let job = LoopJob::new_prompt("p1".into(), "test".into(), "do something".into(), 0, true);
        let msg = build_loop_message(&job);
        let text = match &msg {
            Message::User(u) => u
                .content
                .iter()
                .filter_map(|b| {
                    if let pick_ai::types::ContentBlock::Text(t) = b {
                        Some(t.text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(" "),
            _ => String::new(),
        };
        assert!(text.contains("do something"));
    }
}
