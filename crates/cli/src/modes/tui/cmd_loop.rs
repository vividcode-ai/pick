use pick_loop::commands::{LoopCommand, parse_loop_command};
use pick_loop::types::{LoopJob, LoopJobStatus};

use super::context::TuiContext;

/// Handle /loop, /loop-goal, /loop-status, /loop-pause, etc.
pub(crate) async fn handle_loop(ctx: &mut TuiContext, cmd_name: &str, args: &[String]) {
    let input = args.join(" ").trim().to_string();

    // If command is a specific sub-command, prepend it for parsing
    let full_input = match cmd_name {
        "loop" => input.clone(),
        "loop-goal" => format!("goal {}", input),
        "loop-ask" => format!("ask {}", input),
        "loop-command" | "loop-cmd" => format!("command {}", input),
        "loop-shell" => format!("shell {}", input),
        "loop-status" => "status".to_string(),
        "loop-pause" => format!("pause {}", input),
        "loop-resume" => format!("resume {}", input),
        "loop-remove" => format!("remove {}", input),
        "loop-clear" => "clear".to_string(),
        "loop-now" => format!("now {}", input),
        "loop-stop" => "stop".to_string(),
        "loop-help" => "help".to_string(),
        // Goal subcommands handled above (match cmd_name)
        _ if cmd_name.starts_with("loop-goal-") => input.clone(),
        _ => input.clone(),
    };

    // Handle goal subcommands directly (not via LoopCommand parser)
    match cmd_name {
        "loop-goal-status" => {
            let mgr = ctx.loop_manager.read().await;
            let goals: Vec<_> = mgr.list().iter().filter(|j| j.is_goal()).collect();
            if goals.is_empty() {
                ctx.tui.chat.add_system_message("No goal-driven loop jobs.");
                return;
            }
            ctx.tui
                .chat
                .add_system_message("\x1b[1mGoal Loop Jobs:\x1b[0m");
            for job in &goals {
                let status = job.goal_status.as_deref().unwrap_or("active");
                ctx.tui.chat.add_system_message(&format!(
                    "  {}. \x1b[1m{}\x1b[0m  status: {}  runs: {}",
                    job.name, job.action, status, job.run_count
                ));
            }
            return;
        }
        "loop-goal-pause" => {
            let target = if input.is_empty() {
                None
            } else {
                Some(input.as_str())
            };
            let mut mgr = ctx.loop_manager.write().await;
            let paused: Vec<String> = mgr
                .list()
                .iter()
                .filter(|j| j.is_goal() && (target.is_none() || target == Some(&j.id)))
                .map(|j| j.id.clone())
                .collect();
            for id in &paused {
                let _ = mgr.pause(id);
            }
            if !paused.is_empty() {
                let _ = mgr.save();
                ctx.tui.chat.add_system_message(&format!(
                    "\x1b[33mPaused {} goal(s).\x1b[0m",
                    paused.len()
                ));
            } else {
                ctx.tui
                    .chat
                    .add_system_message("\x1b[31mNo matching goal loop found.\x1b[0m");
            }
            return;
        }
        "loop-goal-resume" => {
            let target = if input.is_empty() {
                None
            } else {
                Some(input.as_str())
            };
            let mut mgr = ctx.loop_manager.write().await;
            let ids: Vec<String> = mgr
                .list()
                .iter()
                .filter(|j| j.is_goal() && (target.is_none() || target == Some(&j.id)))
                .map(|j| j.id.clone())
                .collect();
            for id in &ids {
                if let Some(job) = mgr.get_mut(id) {
                    job.status = pick_loop::LoopJobStatus::Idle;
                    job.goal_status = Some("active".to_string());
                }
            }
            if !ids.is_empty() {
                let _ = mgr.save();
                ctx.tui
                    .chat
                    .add_system_message(&format!("\x1b[32mResumed {} goal(s).\x1b[0m", ids.len()));
            } else {
                ctx.tui
                    .chat
                    .add_system_message("\x1b[31mNo matching goal loop found.\x1b[0m");
            }
            return;
        }
        "loop-goal-clear" => {
            let mut mgr = ctx.loop_manager.write().await;
            mgr.clear();
            let _ = mgr.save();
            ctx.tui
                .chat
                .add_system_message(&format!("\x1b[33mCleared all goal loops.\x1b[0m"));
            return;
        }
        "loop-goal-done" | "loop-goal-complete" => {
            let summary = if input.is_empty() {
                "Goal completed via CLI"
            } else {
                &input
            };
            let mut mgr = ctx.loop_manager.write().await;
            let target_id = mgr
                .list()
                .iter()
                .find(|j| j.is_goal() && j.status == pick_loop::LoopJobStatus::Running)
                .map(|j| j.id.clone());
            if let Some(id) = target_id {
                if let Some(job) = mgr.get_mut(&id) {
                    job.goal_status = Some("completed".to_string());
                    job.status = pick_loop::LoopJobStatus::Done;
                    job.goal_progress.push(format!("COMPLETED: {}", summary));
                }
                let _ = mgr.save();
                ctx.tui
                    .chat
                    .add_system_message(&format!("\x1b[32m✓ Goal completed: {}\x1b[0m", summary));
            } else {
                ctx.tui
                    .chat
                    .add_system_message("\x1b[31mNo active running goal loop.\x1b[0m");
            }
            return;
        }
        "loop-goal-blocked" => {
            let reason = if input.is_empty() {
                "Blocked via CLI"
            } else {
                &input
            };
            let mut mgr = ctx.loop_manager.write().await;
            let target_id = mgr
                .list()
                .iter()
                .find(|j| j.is_goal() && j.status == pick_loop::LoopJobStatus::Running)
                .map(|j| j.id.clone());
            if let Some(id) = target_id {
                if let Some(job) = mgr.get_mut(&id) {
                    job.goal_status = Some("blocked".to_string());
                    job.status = pick_loop::LoopJobStatus::Paused;
                    job.goal_progress.push(format!("BLOCKED: {}", reason));
                }
                let _ = mgr.save();
                ctx.tui
                    .chat
                    .add_system_message(&format!("\x1b[33mGoal blocked: {}\x1b[0m", reason));
            } else {
                ctx.tui
                    .chat
                    .add_system_message("\x1b[31mNo active running goal loop.\x1b[0m");
            }
            return;
        }
        _ => {}
    }

    let cmd = match parse_loop_command(&full_input) {
        Ok(c) => c,
        Err(e) => {
            ctx.tui
                .chat
                .add_system_message(&format!("\x1b[31mError: {}\x1b[0m", e));
            return;
        }
    };

    match cmd {
        LoopCommand::Create {
            interval,
            action,
            kind,
            flags,
        } => {
            let id = uuid::Uuid::now_v7().to_string();
            let name = if action.len() > 30 {
                format!("{}...", action.chars().take(27).collect::<String>())
            } else {
                action.clone()
            };
            let interval_ms = interval.as_millis() as u64;

            // For goal loops, parse "||" to split objective from acceptance criteria
            let (objective, acceptance) = if kind == "goal" || cmd_name == "loop-goal" {
                if let Some(pos) = action.find(" || ") {
                    let obj = action[..pos].trim().to_string();
                    let crit = action[pos + 4..].trim().to_string();
                    (obj, vec![crit])
                } else {
                    (action.clone(), vec![])
                }
            } else {
                (action.clone(), vec![])
            };

            let mut job = if kind == "goal" || cmd_name == "loop-goal" {
                LoopJob::new_goal(id, objective, acceptance, vec![], interval_ms)
            } else if cmd_name == "loop-shell" {
                let mut j = LoopJob::new_prompt(
                    uuid::Uuid::now_v7().to_string(),
                    name.clone(),
                    action,
                    interval_ms,
                    true,
                );
                j.kind = "shell".to_string();
                j
            } else if cmd_name == "loop-command" || cmd_name == "loop-cmd" {
                let mut j = LoopJob::new_prompt(
                    uuid::Uuid::now_v7().to_string(),
                    name.clone(),
                    action,
                    interval_ms,
                    false, // command loops wait for first interval
                );
                j.kind = "command".to_string();
                j
            } else if cmd_name == "loop-ask" {
                LoopJob::new_prompt(
                    uuid::Uuid::now_v7().to_string(),
                    name.clone(),
                    action,
                    interval_ms,
                    false, // ask waits for first interval
                )
            } else {
                LoopJob::new_prompt(
                    uuid::Uuid::now_v7().to_string(),
                    name.clone(),
                    action,
                    interval_ms,
                    true,
                )
            };

            // Apply flags from command line
            job.safe = flags.safe;
            job.quiet = flags.quiet;
            job.ask_never = flags.ask_never;
            job.no_overlap = flags.no_overlap;
            job.git_checkpoint = flags.git_checkpoint;
            job.max_runs = flags.max_runs;
            job.max_failures = flags.max_failures;
            job.verify_command = flags.verify;
            job.preflight_command = flags.preflight;
            job.postrun_command = flags.postrun;
            job.branch = flags.branch;

            // Store in manager
            let job_id = {
                let mut mgr = ctx.loop_manager.write().await;
                let id = mgr.create(job);
                let _ = mgr.save();
                id
            };

            // Schedule & trigger
            if let Some(ref scheduler) = ctx.loop_scheduler {
                let job_opt = ctx.loop_manager.read().await.get(&job_id).cloned();
                if let Some(j) = job_opt {
                    scheduler.schedule(&j).await;
                    // For idle-driven jobs (interval=0), trigger immediately
                    // so the first run isn't delayed by the 5s watchdog
                    if interval_ms == 0 {
                        scheduler.trigger_job(&job_id).await;
                    }
                }
            }

            ctx.tui.chat.add_system_message(&format!(
                "\x1b[32m✓ Loop job created:\x1b[0m  {} (interval: {:?})",
                name, interval
            ));

            // Update status bar
            update_status_bar(ctx).await;
        }

        LoopCommand::Status => {
            let mgr = ctx.loop_manager.read().await;
            let jobs = mgr.list();
            if jobs.is_empty() {
                ctx.tui.chat.add_system_message("No active loop jobs.");
                return;
            }
            ctx.tui
                .chat
                .add_system_message(&format!("\x1b[1mLoop Jobs ({})\x1b[0m", jobs.len()));
            for (i, job) in jobs.iter().enumerate() {
                let status_label = match job.status {
                    LoopJobStatus::Idle => "\x1b[32midle\x1b[0m",
                    LoopJobStatus::Running => "\x1b[34mrunning\x1b[0m",
                    LoopJobStatus::Paused => "\x1b[33mpaused\x1b[0m",
                    LoopJobStatus::Done => "\x1b[36mdone\x1b[0m",
                    LoopJobStatus::Failed => "\x1b[31mfailed\x1b[0m",
                };
                let due_str = if job.interval_ms == 0 {
                    "idle-triggered".to_string()
                } else {
                    let due_ms = job.due_in_ms(chrono::Utc::now().timestamp_millis());
                    if due_ms <= 0 {
                        "now".to_string()
                    } else {
                        format!("{}s", due_ms / 1000)
                    }
                };
                ctx.tui.chat.add_system_message(&format!(
                    "  {}. \x1b[1m{}\x1b[0m  ({})  {}  runs: {}/{}  next: {}",
                    i + 1,
                    job.name,
                    job.kind,
                    status_label,
                    job.run_count,
                    job.max_runs
                        .map(|m| m.to_string())
                        .unwrap_or_else(|| "∞".into()),
                    due_str,
                ));
                ctx.tui.chat.add_system_message(&format!(
                    "     action: {}",
                    if job.action.len() > 60 {
                        format!("{}...", job.action.chars().take(57).collect::<String>())
                    } else {
                        job.action.clone()
                    }
                ));
            }
            ctx.tui.chat.add_system_message(
                "/loop-pause <id>  /loop-resume <id>  /loop-remove <id>  /loop-clear",
            );
        }

        LoopCommand::Pause { job_id } => {
            let id = job_id.unwrap_or_default();
            let result = {
                let mut mgr = ctx.loop_manager.write().await;
                let r = mgr.pause(&id);
                if r.is_ok() {
                    let _ = mgr.save();
                }
                r
            };
            if let Some(ref scheduler) = ctx.loop_scheduler {
                scheduler.deschedule(&id).await;
            }
            match result {
                Ok(()) => ctx
                    .tui
                    .chat
                    .add_system_message(&format!("\x1b[33mPaused loop job: {}\x1b[0m", id)),
                Err(e) => ctx
                    .tui
                    .chat
                    .add_system_message(&format!("\x1b[31m{}\x1b[0m", e)),
            }
            update_status_bar(ctx).await;
        }

        LoopCommand::Resume { job_id } => {
            let id = job_id.unwrap_or_default();
            let (result, job_opt) = {
                let mut mgr = ctx.loop_manager.write().await;
                let r = mgr.resume(&id);
                if r.is_ok() {
                    let _ = mgr.save();
                }
                let job = mgr.get(&id).cloned();
                (r, job)
            };
            if let Some(ref scheduler) = ctx.loop_scheduler {
                if let Some(job) = job_opt {
                    scheduler.schedule(&job).await;
                }
            }
            match result {
                Ok(()) => ctx
                    .tui
                    .chat
                    .add_system_message(&format!("\x1b[32mResumed loop job: {}\x1b[0m", id)),
                Err(e) => ctx
                    .tui
                    .chat
                    .add_system_message(&format!("\x1b[31m{}\x1b[0m", e)),
            }
            update_status_bar(ctx).await;
        }

        LoopCommand::Remove { job_id } => {
            let removed = {
                let mut mgr = ctx.loop_manager.write().await;
                let r = mgr.remove(&job_id);
                if r {
                    let _ = mgr.save();
                }
                r
            };
            if let Some(ref scheduler) = ctx.loop_scheduler {
                scheduler.deschedule(&job_id).await;
            }
            if removed {
                ctx.tui
                    .chat
                    .add_system_message(&format!("\x1b[32mRemoved loop job: {}\x1b[0m", job_id));
            } else {
                ctx.tui
                    .chat
                    .add_system_message(&format!("\x1b[31mJob not found: {}\x1b[0m", job_id));
            }
            update_status_bar(ctx).await;
        }

        LoopCommand::Clear => {
            {
                let mut mgr = ctx.loop_manager.write().await;
                mgr.clear();
                let _ = mgr.save();
            }
            if let Some(ref scheduler) = ctx.loop_scheduler {
                scheduler.deschedule_all().await;
            }
            ctx.tui
                .chat
                .add_system_message("\x1b[33mAll loop jobs cleared.\x1b[0m");
            update_status_bar(ctx).await;
        }

        LoopCommand::Now { job_id } => {
            if let Some(ref scheduler) = ctx.loop_scheduler {
                // Trigger the job immediately
                let name = {
                    let mgr = ctx.loop_manager.read().await;
                    mgr.get(&job_id).map(|j| j.name.clone())
                };
                scheduler.trigger_job(&job_id).await;
                ctx.tui.chat.add_system_message(&format!(
                    "\x1b[34mTriggered loop job: {}\x1b[0m",
                    name.as_deref().unwrap_or(&job_id)
                ));
                update_status_bar(ctx).await;
            }
        }

        LoopCommand::Stop => {
            ctx.tui
                .chat
                .add_system_message("\x1b[33mLoop stop requested.\x1b[0m");
            if let Some(ref scheduler) = ctx.loop_scheduler {
                scheduler.stop_watchdog();
                scheduler.deschedule_all().await;
            }
            update_status_bar(ctx).await;
        }

        LoopCommand::Help => {
            display_help(ctx);
        }
    }
}

/// Display loop help.
fn display_help(ctx: &mut TuiContext) {
    ctx.tui.chat.add_system_message(
        "\x1b[1mLoop Commands:\x1b[0m

  \x1b[33m/loop <interval> <prompt>\x1b[0m
    Create a loop that sends a prompt on an interval.
    Examples:
      /loop 30s fix the build
      /loop 5m check for updates
      /loop 0 watch and fix   (runs every time agent is idle)

  \x1b[33m/loop-goal <interval> <objective>\x1b[0m
    Create a goal-driven loop with completion tools.

  \x1b[33m/loop-status\x1b[0m
    Show all loop jobs and their status.

  \x1b[33m/loop-pause <id>\x1b[0m
    Pause a loop job.

  \x1b[33m/loop-resume <id>\x1b[0m
    Resume a paused loop job.

  \x1b[33m/loop-remove <id>\x1b[0m
    Remove a loop job.

  \x1b[33m/loop-clear\x1b[0m
    Remove all loop jobs.

  \x1b[33m/loop-now <id>\x1b[0m
    Trigger a loop job immediately.

  \x1b[33m/loop-stop\x1b[0m
    Stop the watchdog/scheduler.

  Interval formats: 30s (seconds), 5m (minutes), 1h (hours), 0 (idle)",
    );
}

/// Update the TUI status bar with loop job counts.
async fn update_status_bar(ctx: &mut TuiContext) {
    let mgr = ctx.loop_manager.read().await;
    let total = mgr.list().len();
    if total > 0 {
        // Populate detailed loop job info for display above editor
        let jobs: Vec<pick_loop::types::LoopJobStatusInfo> = mgr
            .list()
            .iter()
            .map(pick_loop::types::LoopJobStatusInfo::from)
            .collect();
        let jobs_json: Vec<serde_json::Value> = jobs
            .into_iter()
            .filter_map(|j| serde_json::to_value(j).ok())
            .collect();
        ctx.tui.set_loop_jobs(jobs_json);
    } else {
        ctx.tui.set_loop_status(None);
        ctx.tui.set_loop_jobs(Vec::new());
    }
}
