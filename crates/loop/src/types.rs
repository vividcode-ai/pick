use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Status machine for a loop job.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LoopJobStatus {
    /// Waiting for the next trigger (timer or idle event).
    Idle,
    /// Currently being executed by the agent loop.
    Running,
    /// Manually paused by user via `/loop-pause`.
    Paused,
    /// Completed (maxRuns reached, goal completed, stop condition met).
    Done,
    /// Exceeded maxFailures.
    Failed,
}

impl LoopJobStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, LoopJobStatus::Idle | LoopJobStatus::Running)
    }

    pub fn label(&self) -> &'static str {
        match self {
            LoopJobStatus::Idle => "idle",
            LoopJobStatus::Running => "running",
            LoopJobStatus::Paused => "paused",
            LoopJobStatus::Done => "done",
            LoopJobStatus::Failed => "failed",
        }
    }
}

/// A single loop job — analogous to opencode-loop's `job` object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopJob {
    // --- Identity ---
    pub id: String,
    pub name: String,
    pub kind: String, // "prompt" | "command" | "shell" | "goal" | "compact"

    // --- Core action ---
    pub action: String,

    // --- Timing ---
    pub interval_ms: u64, // 0 = fire on every idle
    pub immediate: bool,  // fire immediately after creation

    // --- Stop conditions ---
    pub max_runs: Option<u32>,
    pub max_runtime_ms: Option<u64>,
    pub max_failures: Option<u32>,
    pub timeout_ms: Option<u64>,

    // --- File-based stop ---
    pub stop_file: Option<PathBuf>,
    pub until: Option<String>,
    pub progress_file: Option<PathBuf>,

    // --- Lifecycle hooks (shell commands) ---
    pub preflight_command: Option<String>,
    pub verify_command: Option<String>,
    pub postrun_command: Option<String>,

    // --- Git ---
    pub branch: Option<String>,
    pub git_checkpoint: bool,
    pub checkpoint_only: bool,

    // --- Behaviour flags ---
    pub safe: bool,
    pub quiet: bool,
    pub ask_never: bool,
    pub no_overlap: bool,
    pub batch: Option<u32>,

    // --- Watch ---
    pub watch_paths: Vec<PathBuf>,
    /// Snapshot of watched files: path -> "mtime_ms:len" for change detection.
    pub watch_snapshot: HashMap<String, String>,

    // --- Goal mode ---
    pub goal_status: Option<String>,
    pub goal_acceptance: Vec<String>,
    pub goal_checks: Vec<String>,
    pub goal_progress: Vec<String>,

    // --- Runtime tracking ---
    pub status: LoopJobStatus,
    pub run_count: u32,
    pub failure_count: u32,
    pub last_run_at: Option<i64>, // epoch ms
    pub last_verify_failure: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl LoopJob {
    /// Create a new prompt-type loop job.
    pub fn new_prompt(
        id: String,
        name: String,
        action: String,
        interval_ms: u64,
        immediate: bool,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id,
            name,
            kind: "prompt".into(),
            action,
            interval_ms,
            immediate,
            max_runs: None,
            max_runtime_ms: None,
            max_failures: None,
            timeout_ms: None,
            stop_file: None,
            until: None,
            progress_file: None,
            preflight_command: None,
            verify_command: None,
            postrun_command: None,
            branch: None,
            git_checkpoint: false,
            checkpoint_only: false,
            safe: false,
            quiet: false,
            ask_never: false,
            no_overlap: true,
            batch: None,
            watch_paths: Vec::new(),
            watch_snapshot: HashMap::new(),
            goal_status: None,
            goal_acceptance: Vec::new(),
            goal_checks: Vec::new(),
            goal_progress: Vec::new(),
            status: LoopJobStatus::Idle,
            run_count: 0,
            failure_count: 0,
            last_run_at: if immediate { None } else { Some(now) },
            last_verify_failure: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a new goal-type loop job.
    pub fn new_goal(
        id: String,
        action: String,
        acceptance: Vec<String>,
        checks: Vec<String>,
        interval_ms: u64,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id,
            name: "goal".into(),
            kind: "goal".into(),
            action,
            interval_ms,
            immediate: true,
            max_runs: None,
            max_runtime_ms: None,
            max_failures: None,
            timeout_ms: None,
            stop_file: None,
            until: None,
            progress_file: None,
            preflight_command: None,
            verify_command: None,
            postrun_command: None,
            branch: None,
            git_checkpoint: false,
            checkpoint_only: false,
            safe: true,
            quiet: false,
            ask_never: true,
            no_overlap: true,
            batch: None,
            watch_paths: Vec::new(),
            watch_snapshot: HashMap::new(),
            goal_status: Some("active".into()),
            goal_acceptance: acceptance,
            goal_checks: checks,
            goal_progress: Vec::new(),
            status: LoopJobStatus::Idle,
            run_count: 0,
            failure_count: 0,
            last_run_at: None,
            last_verify_failure: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Returns true if the job is a goal.
    pub fn is_goal(&self) -> bool {
        self.kind == "goal" || self.goal_status.is_some()
    }

    /// Time in ms until the next due time (0 = due now).
    pub fn due_in_ms(&self, now_ms: i64) -> i64 {
        if !self.status.is_active() {
            return i64::MAX;
        }
        if self.interval_ms == 0 {
            return 0; // idle-driven
        }
        match self.last_run_at {
            Some(last) => {
                let next = last + self.interval_ms as i64;
                (next - now_ms).max(0)
            }
            None => 0, // never run → due now
        }
    }

    /// Whether the job is due at the given time.
    pub fn is_due(&self, now_ms: i64) -> bool {
        self.due_in_ms(now_ms) <= 0
    }

    /// Whether maxRuns has been reached.
    pub fn max_runs_reached(&self) -> bool {
        self.max_runs
            .map(|max| self.run_count >= max)
            .unwrap_or(false)
    }

    /// Whether maxRuntime has been exceeded.
    pub fn max_runtime_exceeded(&self, now_ms: i64) -> bool {
        self.max_runtime_ms
            .map(|max| (now_ms - self.created_at) >= max as i64)
            .unwrap_or(false)
    }
}

/// Persisted loop store (versioned).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopStore {
    pub version: u32,
    pub jobs: Vec<LoopJob>,
}

impl LoopStore {
    pub const CURRENT_VERSION: u32 = 1;
}

/// Status info struct sent to UI (TUI/Web) for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopJobStatusInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub status: String,
    pub run_count: u32,
    pub max_runs: Option<u32>,
    pub failure_count: u32,
    pub max_failures: Option<u32>,
    pub interval_ms: u64,
    pub next_due_ms: i64,
    pub last_run_at: Option<i64>,
    pub created_at: i64,
}

impl From<&LoopJob> for LoopJobStatusInfo {
    fn from(job: &LoopJob) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id: job.id.clone(),
            name: job.name.clone(),
            kind: job.kind.clone(),
            status: job.status.label().to_string(),
            run_count: job.run_count,
            max_runs: job.max_runs,
            failure_count: job.failure_count,
            max_failures: job.max_failures,
            interval_ms: job.interval_ms,
            next_due_ms: job.due_in_ms(now),
            last_run_at: job.last_run_at,
            created_at: job.created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_prompt_job() {
        let job = LoopJob::new_prompt(
            "test-1".into(),
            "fix-build".into(),
            "fix the build".into(),
            300_000,
            true,
        );
        assert_eq!(job.kind, "prompt");
        assert_eq!(job.status, LoopJobStatus::Idle);
        assert!(job.last_run_at.is_none()); // immediate => None
    }

    #[test]
    fn test_new_goal_job() {
        let job = LoopJob::new_goal(
            "goal-1".into(),
            "refactor".into(),
            vec!["all tests pass".into()],
            vec!["cargo test".into()],
            0,
        );
        assert_eq!(job.kind, "goal");
        assert_eq!(job.goal_status, Some("active".into()));
        assert!(job.safe);
        assert!(job.ask_never);
    }

    #[test]
    fn test_due_in_ms() {
        let now = chrono::Utc::now().timestamp_millis();
        let mut job = LoopJob::new_prompt("t".into(), "".into(), "".into(), 100_000, false);
        // immediate=false, last_run_at=now
        assert!(job.last_run_at.is_some());
        job.last_run_at = Some(now - 50_000);
        // 50_000ms have elapsed, interval=100_000, so 50_000 remaining
        assert_eq!(job.due_in_ms(now), 50_000);
        // overdue
        job.last_run_at = Some(now - 150_000);
        assert_eq!(job.due_in_ms(now), 0);
    }

    #[test]
    fn test_max_runs_reached() {
        let mut job = LoopJob::new_prompt("t".into(), "".into(), "".into(), 0, true);
        job.max_runs = Some(5);
        job.run_count = 5;
        assert!(job.max_runs_reached());
        job.run_count = 4;
        assert!(!job.max_runs_reached());
    }

    #[test]
    fn test_status_label() {
        assert_eq!(LoopJobStatus::Idle.label(), "idle");
        assert_eq!(LoopJobStatus::Running.label(), "running");
        assert_eq!(LoopJobStatus::Paused.label(), "paused");
        assert_eq!(LoopJobStatus::Done.label(), "done");
        assert_eq!(LoopJobStatus::Failed.label(), "failed");
    }

    #[test]
    fn test_is_active() {
        assert!(LoopJobStatus::Idle.is_active());
        assert!(LoopJobStatus::Running.is_active());
        assert!(!LoopJobStatus::Paused.is_active());
        assert!(!LoopJobStatus::Done.is_active());
        assert!(!LoopJobStatus::Failed.is_active());
    }

    #[test]
    fn test_loop_job_status_info_from() {
        let job = LoopJob::new_prompt("x".into(), "test".into(), "action".into(), 5000, true);
        let info = LoopJobStatusInfo::from(&job);
        assert_eq!(info.id, "x");
        assert_eq!(info.name, "test");
        assert_eq!(info.status, "idle");
    }
}
