use std::path::{Path, PathBuf};

use tracing::warn;

use crate::types::{LoopJob, LoopJobStatus, LoopStore};

/// Manages loop jobs for a single session.
/// Thread-safe: intended to be used behind `Arc<RwLock<LoopManager>>`.
#[derive(Debug, Clone)]
pub struct LoopManager {
    jobs: Vec<LoopJob>,
    persistence_path: PathBuf,
}

impl LoopManager {
    /// Create a new empty manager.
    pub fn new(persistence_path: PathBuf) -> Self {
        Self {
            jobs: Vec::new(),
            persistence_path,
        }
    }

    // ── CRUD ────────────────────────────────────────────────────────────────

    /// Add a job. If a job with the same `name` exists, replaces it (unless
    /// the incoming job has `multi` semantics — currently always replaces).
    /// Returns the job ID.
    pub fn create(&mut self, job: LoopJob) -> String {
        let id = job.id.clone();
        // Replace existing job with same name
        if let Some(pos) = self.jobs.iter().position(|j| j.name == job.name) {
            self.jobs[pos] = job;
        } else {
            self.jobs.push(job);
        }
        id
    }

    /// Get a reference to a job by ID.
    pub fn get(&self, id: &str) -> Option<&LoopJob> {
        self.jobs.iter().find(|j| j.id == id)
    }

    /// Get a mutable reference to a job by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut LoopJob> {
        self.jobs.iter_mut().find(|j| j.id == id)
    }

    /// Remove a job by ID. Returns true if found.
    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.jobs.len();
        self.jobs.retain(|j| j.id != id);
        self.jobs.len() < len
    }

    /// Remove all jobs.
    pub fn clear(&mut self) {
        self.jobs.clear();
    }

    /// List all jobs.
    pub fn list(&self) -> &[LoopJob] {
        &self.jobs
    }

    /// List all jobs (mutable).
    pub fn list_mut(&mut self) -> &mut Vec<LoopJob> {
        &mut self.jobs
    }

    /// Count of jobs that are active (Idle or Running).
    pub fn active_count(&self) -> usize {
        self.jobs.iter().filter(|j| j.status.is_active()).count()
    }

    /// Returns true if there are any active jobs.
    pub fn has_active(&self) -> bool {
        self.active_count() > 0
    }

    /// Returns all jobs that are due at the given timestamp.
    pub fn due_jobs(&self, now_ms: i64) -> Vec<&LoopJob> {
        self.jobs
            .iter()
            .filter(|j| {
                j.status.is_active()
                    && !j.max_runs_reached()
                    && !j.max_runtime_exceeded(now_ms)
                    && j.is_due(now_ms)
                    // interval_ms=0 jobs fire once on initial trigger; skip
                    // auto-re-trigger for jobs that already ran to prevent
                    // runaway watchdog / scheduler cycling.
                    && (j.interval_ms > 0 || j.run_count == 0)
            })
            .collect()
    }

    /// Minimum time (in ms) until the next job is due.
    /// Returns `None` if no jobs are scheduled.
    pub fn next_due_delay_ms(&self, now_ms: i64) -> Option<i64> {
        self.jobs
            .iter()
            .filter(|j| j.status.is_active() && !j.max_runs_reached())
            .map(|j| j.due_in_ms(now_ms))
            .min()
    }

    // ── State transitions ───────────────────────────────────────────────────

    pub fn mark_running(&mut self, id: &str) {
        if let Some(job) = self.get_mut(id) {
            job.status = LoopJobStatus::Running;
            job.updated_at = chrono::Utc::now().timestamp_millis();
        }
    }

    pub fn mark_idle(&mut self, id: &str) {
        if let Some(job) = self.get_mut(id) {
            job.status = LoopJobStatus::Idle;
            job.updated_at = chrono::Utc::now().timestamp_millis();
        }
    }

    pub fn mark_done(&mut self, id: &str) {
        if let Some(job) = self.get_mut(id) {
            job.status = LoopJobStatus::Done;
            job.updated_at = chrono::Utc::now().timestamp_millis();
            if job.is_goal() {
                job.goal_status = Some("completed".into());
            }
        }
    }

    pub fn mark_failed(&mut self, id: &str) {
        if let Some(job) = self.get_mut(id) {
            job.status = LoopJobStatus::Failed;
            job.updated_at = chrono::Utc::now().timestamp_millis();
        }
    }

    pub fn pause(&mut self, id: &str) -> Result<(), String> {
        match self.get_mut(id) {
            Some(job)
                if job.status == LoopJobStatus::Idle || job.status == LoopJobStatus::Running =>
            {
                job.status = LoopJobStatus::Paused;
                job.updated_at = chrono::Utc::now().timestamp_millis();
                Ok(())
            }
            Some(_) => Err(format!(
                "Job {} is not running (status: {:?})",
                id,
                self.get(id).map(|j| &j.status)
            )),
            None => Err(format!("Job {} not found", id)),
        }
    }

    pub fn resume(&mut self, id: &str) -> Result<(), String> {
        match self.get_mut(id) {
            Some(job) if job.status == LoopJobStatus::Paused => {
                job.status = LoopJobStatus::Idle;
                job.last_run_at = None; // reset so it's due immediately
                job.updated_at = chrono::Utc::now().timestamp_millis();
                Ok(())
            }
            Some(_) => Err(format!("Job {} is not paused", id)),
            None => Err(format!("Job {} not found", id)),
        }
    }

    // ── Runtime tracking ────────────────────────────────────────────────────

    /// Record a completed run. Returns true if maxRuns was reached.
    pub fn record_run(&mut self, id: &str) -> bool {
        let now = chrono::Utc::now().timestamp_millis();
        if let Some(job) = self.get_mut(id) {
            job.run_count += 1;
            job.last_run_at = Some(now);
            job.updated_at = now;
            if job.max_runs_reached() {
                job.status = LoopJobStatus::Done;
                return true;
            }
        }
        false
    }

    /// Record a failure. Returns true if maxFailures was exceeded.
    pub fn record_failure(&mut self, id: &str, reason: Option<String>) -> bool {
        let now = chrono::Utc::now().timestamp_millis();
        if let Some(job) = self.get_mut(id) {
            job.failure_count += 1;
            if let Some(r) = reason {
                job.last_verify_failure = Some(r);
            }
            job.updated_at = now;
            if let Some(max) = job.max_failures {
                if job.failure_count >= max {
                    job.status = LoopJobStatus::Failed;
                    return true;
                }
            }
        }
        false
    }

    // ── Persistence ─────────────────────────────────────────────────────────

    /// Save all jobs to the persistence file.
    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.persistence_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create loops dir: {}", e))?;
        }
        let store = LoopStore {
            version: LoopStore::CURRENT_VERSION,
            jobs: self.jobs.clone(),
        };
        let json = serde_json::to_string_pretty(&store)
            .map_err(|e| format!("Failed to serialize loop store: {}", e))?;
        std::fs::write(&self.persistence_path, &json)
            .map_err(|e| format!("Failed to write loop state: {}", e))?;
        Ok(())
    }

    /// Load jobs from the persistence file. Returns a new manager.
    pub fn load(path: &Path) -> Self {
        if !path.exists() {
            return Self::new(path.to_path_buf());
        }
        let json = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to read loop state file ({}): {}", path.display(), e);
                return Self::new(path.to_path_buf());
            }
        };
        let store: LoopStore = match serde_json::from_str(&json) {
            Ok(s) => s,
            Err(e) => {
                warn!(
                    "Failed to parse loop state file ({}): {}",
                    path.display(),
                    e
                );
                return Self::new(path.to_path_buf());
            }
        };
        Self {
            jobs: store.jobs,
            persistence_path: path.to_path_buf(),
        }
    }

    /// Deactivate stale running jobs (if agent crashed).
    pub fn recover_stale_runs(&mut self, max_age_ms: i64) {
        let now = chrono::Utc::now().timestamp_millis();
        for job in &mut self.jobs {
            if job.status == LoopJobStatus::Running {
                if let Some(last) = job.last_run_at {
                    if now - last > max_age_ms {
                        warn!(
                            "Recovering stale running job {} (last_run={}, age={}ms)",
                            job.id,
                            last,
                            now - last
                        );
                        job.status = LoopJobStatus::Idle;
                        job.failure_count += 1;
                    }
                }
            }
        }
    }
}

pub fn loops_dir(cwd: &Path) -> PathBuf {
    cwd.join(".pick").join("loops")
}

pub fn loops_path_for_session(cwd: &Path, session_id: &str) -> PathBuf {
    loops_dir(cwd).join(format!("{}.json", session_id))
}

/// Load a LoopManager for the given session.
pub fn load_loop_manager(cwd: &Path, session_id: &str) -> LoopManager {
    let path = loops_path_for_session(cwd, session_id);
    LoopManager::load(&path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LoopJob;
    use tempfile::TempDir;

    fn test_mgr() -> LoopManager {
        LoopManager::new(PathBuf::from("/tmp/test.json"))
    }

    fn test_job(name: &str) -> LoopJob {
        LoopJob::new_prompt(
            uuid::Uuid::now_v7().to_string(),
            name.into(),
            "action".into(),
            0,
            true,
        )
    }

    #[test]
    fn test_create_and_get() {
        let mut mgr = test_mgr();
        let job = test_job("test");
        let id = mgr.create(job);
        assert!(mgr.get(&id).is_some());
    }

    #[test]
    fn test_create_replaces_same_name() {
        let mut mgr = test_mgr();
        let j1 = test_job("foo");
        let j2 = LoopJob::new_prompt(
            uuid::Uuid::now_v7().to_string(),
            "foo".into(),
            "replaced".into(),
            0,
            true,
        );
        let _id1 = mgr.create(j1);
        let _id2 = mgr.create(j2);
        // Only one job with name "foo" should exist
        assert_eq!(mgr.list().len(), 1);
        assert_eq!(mgr.list()[0].action, "replaced");
    }

    #[test]
    fn test_remove() {
        let mut mgr = test_mgr();
        let j = test_job("x");
        let id = mgr.create(j);
        assert!(mgr.remove(&id));
        assert!(!mgr.remove(&id));
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_clear() {
        let mut mgr = test_mgr();
        mgr.create(test_job("a"));
        mgr.create(test_job("b"));
        mgr.clear();
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_state_transitions() {
        let mut mgr = test_mgr();
        let j = test_job("x");
        let id = mgr.create(j);

        mgr.mark_running(&id);
        assert_eq!(mgr.get(&id).unwrap().status, LoopJobStatus::Running);

        mgr.mark_idle(&id);
        assert_eq!(mgr.get(&id).unwrap().status, LoopJobStatus::Idle);

        mgr.mark_done(&id);
        assert_eq!(mgr.get(&id).unwrap().status, LoopJobStatus::Done);
    }

    #[test]
    fn test_pause_resume() {
        let mut mgr = test_mgr();
        let j = test_job("x");
        let id = mgr.create(j);

        assert!(mgr.pause(&id).is_ok());
        assert_eq!(mgr.get(&id).unwrap().status, LoopJobStatus::Paused);

        assert!(mgr.resume(&id).is_ok());
        assert_eq!(mgr.get(&id).unwrap().status, LoopJobStatus::Idle);
    }

    #[test]
    fn test_pause_fails_on_done() {
        let mut mgr = test_mgr();
        let j = test_job("x");
        let id = mgr.create(j);
        mgr.mark_done(&id);
        assert!(mgr.pause(&id).is_err());
    }

    #[test]
    fn test_record_run() {
        let mut mgr = test_mgr();
        let mut j = test_job("x");
        j.max_runs = Some(3);
        let id = mgr.create(j);

        assert!(!mgr.record_run(&id));
        assert!(!mgr.record_run(&id));
        assert!(mgr.record_run(&id)); // 3rd run, max_runs=3
        assert_eq!(mgr.get(&id).unwrap().status, LoopJobStatus::Done);
    }

    #[test]
    fn test_record_failure() {
        let mut mgr = test_mgr();
        let mut j = test_job("x");
        j.max_failures = Some(2);
        let id = mgr.create(j);

        assert!(!mgr.record_failure(&id, None));
        assert!(mgr.record_failure(&id, None));
        assert_eq!(mgr.get(&id).unwrap().status, LoopJobStatus::Failed);
    }

    #[test]
    fn test_due_jobs() {
        let now = chrono::Utc::now().timestamp_millis();
        let mut mgr = test_mgr();

        // Job with interval=0 (idle-driven) → always due
        let mut j1 = test_job("idle");
        j1.interval_ms = 0;
        mgr.create(j1);

        // Job with interval=10s, run 5s ago → due in 5s
        let mut j2 = test_job("not-yet");
        j2.interval_ms = 10_000;
        j2.last_run_at = Some(now - 5_000);
        mgr.create(j2);

        // Job with interval=10s, run 15s ago → overdue
        let mut j3 = test_job("overdue");
        j3.interval_ms = 10_000;
        j3.last_run_at = Some(now - 15_000);
        mgr.create(j3);

        let due = mgr.due_jobs(now);
        assert_eq!(due.len(), 2);
        assert!(due.iter().any(|j| j.name == "idle"));
        assert!(due.iter().any(|j| j.name == "overdue"));
    }

    #[test]
    fn test_save_and_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("loops").join("sess-1.json");

        // Create and save
        {
            let mut mgr = LoopManager::new(path.clone());
            mgr.create(test_job("a"));
            mgr.create(test_job("b"));
            mgr.save().unwrap();
        }

        // Load and verify
        let mgr = LoopManager::load(&path);
        assert_eq!(mgr.list().len(), 2);
        assert_eq!(mgr.list()[0].name, "a");
        assert_eq!(mgr.list()[1].name, "b");
    }

    #[test]
    fn test_recover_stale_runs() {
        let now = chrono::Utc::now().timestamp_millis();
        let mut mgr = test_mgr();
        let mut j = test_job("stale");
        j.status = LoopJobStatus::Running;
        j.last_run_at = Some(now - 120_000); // 2 minutes ago
        mgr.create(j);

        mgr.recover_stale_runs(60_000); // 1 minute threshold
        assert_eq!(mgr.list()[0].status, LoopJobStatus::Idle);
        assert_eq!(mgr.list()[0].failure_count, 1);
    }
}
