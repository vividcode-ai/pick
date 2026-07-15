//! Three-layer async scheduler for loop jobs.
//!
//! Architecture (matching opencode-loop):
//! 1. **Event-driven** — `on_session_idle()` triggers `maybe_run_due_jobs()`.
//! 2. **Timer-based**  — per-job `tokio::spawn` with `tokio::time::sleep`.
//! 3. **Watchdog**     — 5-second interval polling backup.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::manager::LoopManager;
use crate::types::{LoopJob, LoopJobStatus};

// ── Type aliases ───────────────────────────────────────────────────────────

/// Callback invoked when the scheduler decides a job should run.
pub type TriggerCallback =
    Arc<dyn Fn(LoopJob) -> Pin<Box<dyn Send + Future<Output = ()>>> + Send + Sync>;

// ── Scheduler ──────────────────────────────────────────────────────────────

pub struct LoopScheduler {
    manager: Arc<RwLock<LoopManager>>,
    /// Per-job timer handles (keyed by job ID).
    /// Uses std::sync::Mutex for interior mutability from &self.
    job_timers: Mutex<HashMap<String, tokio::task::JoinHandle<()>>>,
    /// Run lock — prevents concurrent `maybe_run_due_jobs`.
    run_lock: Arc<AtomicBool>,
    /// Callback to trigger agent execution.
    trigger_cb: Option<TriggerCallback>,
    /// Watchdog handle.
    watchdog_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Shutdown flag.
    shutdown: Arc<AtomicBool>,
    /// Stale run recovery threshold (ms).
    run_lock_timeout_ms: i64,
}

impl LoopScheduler {
    pub fn new(manager: Arc<RwLock<LoopManager>>) -> Self {
        Self {
            manager,
            job_timers: Mutex::new(HashMap::new()),
            run_lock: Arc::new(AtomicBool::new(false)),
            trigger_cb: None,
            watchdog_handle: Mutex::new(None),
            shutdown: Arc::new(AtomicBool::new(false)),
            run_lock_timeout_ms: 45_000,
        }
    }

    /// Set the trigger callback.
    pub fn set_trigger_cb(&mut self, cb: TriggerCallback) {
        self.trigger_cb = Some(cb);
    }

    // ── Job lifecycle ─────────────────────────────────────────────────────

    /// Register a job: schedule its interval timer.
    pub async fn schedule(&self, job: &LoopJob) {
        self.cancel_timer(&job.id);
        if job.interval_ms == 0 {
            return; // Idle-driven: no timer needed
        }
        let handle = self.spawn_job_timer(job);
        self.job_timers
            .lock()
            .unwrap()
            .insert(job.id.clone(), handle);
    }

    /// Remove a job: cancel its timer.
    pub async fn deschedule(&self, job_id: &str) {
        self.cancel_timer(job_id);
    }

    /// Remove all job timers.
    pub async fn deschedule_all(&self) {
        let mut timers = self.job_timers.lock().unwrap();
        for (_, handle) in timers.drain() {
            handle.abort();
        }
    }

    /// Called after a job's agent turn completes.
    pub async fn on_job_turn_complete(&self, job_id: &str) {
        let mut mgr = self.manager.write().await;
        let reached_max = mgr.record_run(job_id);

        if reached_max {
            debug!("Job {} reached max_runs, marking done", job_id);
        } else {
            mgr.mark_idle(job_id);
        }

        // Run verify command (if configured) — execute shell command
        let verify_cmd = mgr.get(job_id).and_then(|j| j.verify_command.clone());
        if let Some(ref cmd) = verify_cmd {
            debug!("Running verify command for job {}: {}", job_id, cmd);
            let output = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .output()
                .await;
            match output {
                Ok(out) => {
                    if !out.status.success() {
                        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                        let exceeded = mgr.record_failure(job_id, Some(stderr));
                        if exceeded {
                            debug!("Job {} exceeded max_failures, marking failed", job_id);
                        }
                    } else {
                        // Clear failure count on success
                        if let Some(job) = mgr.get_mut(job_id) {
                            job.failure_count = 0;
                            job.last_verify_failure = None;
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to run verify command for job {}: {}", job_id, e);
                }
            }
        }

        // Run postrun command (if configured)
        let postrun_cmd = mgr.get(job_id).and_then(|j| j.postrun_command.clone());
        if let Some(ref cmd) = postrun_cmd {
            debug!("Running postrun command for job {}: {}", job_id, cmd);
            let _ = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .output()
                .await;
        }

        let _ = mgr.save();
        drop(mgr);

        // Reschedule if still active
        if let Some(job) = self.manager.read().await.get(job_id).cloned()
            && job.status == LoopJobStatus::Idle
            && !job.max_runs_reached()
        {
            self.schedule(&job).await;
        }
    }

    // ── Main execution ─────────────────────────────────────────────────────

    /// Check for and execute all due jobs. Returns count triggered.
    pub async fn maybe_run_due_jobs(&self) -> usize {
        if self.run_lock.swap(true, Ordering::Acquire) {
            return 0; // Already running
        }
        let result = self.run_due_check().await;
        self.run_lock.store(false, Ordering::Release);
        result
    }

    async fn run_due_check(&self) -> usize {
        let now = chrono::Utc::now().timestamp_millis();

        // Recover stale runs
        {
            let mut mgr = self.manager.write().await;
            mgr.recover_stale_runs(self.run_lock_timeout_ms);
        }

        // Collect due jobs
        let due: Vec<LoopJob> = {
            let mgr = self.manager.read().await;
            mgr.due_jobs(now).iter().map(|j| (*j).clone()).collect()
        };

        if due.is_empty() {
            return 0;
        }

        // Pre-flight checks: stop_file, until, watch_paths
        let mut to_trigger: Vec<LoopJob> = Vec::new();
        {
            let mut mgr = self.manager.write().await;
            for job in &due {
                // Check stop_file
                if let Some(ref stop_file) = job.stop_file
                    && stop_file.exists()
                {
                    debug!(
                        "Stop file '{}' exists for job {}, removing",
                        stop_file.display(),
                        job.id
                    );
                    mgr.mark_done(&job.id);
                    let _ = mgr.save();
                    continue;
                }

                // Check until condition (stored in a status file)
                if let Some(ref until) = job.until
                    && let Ok(content) = std::fs::read_to_string(
                        crate::manager::loops_dir(std::path::Path::new(".")).join("until.txt"),
                    )
                    && content.contains(until)
                {
                    debug!("Until condition matched for job {}, removing", job.id);
                    mgr.mark_done(&job.id);
                    let _ = mgr.save();
                    continue;
                }

                // Check watch_paths: update snapshot and skip if unchanged
                if !job.watch_paths.is_empty() {
                    let mut changed = false;
                    let mut new_snap = std::collections::HashMap::new();
                    for path in &job.watch_paths {
                        if let Ok(meta) = path.metadata() {
                            let key = path.to_string_lossy().to_string();
                            let mtime = meta
                                .modified()
                                .ok()
                                .and_then(|t| t.elapsed().ok())
                                .map(|d| d.as_secs())
                                .unwrap_or(0);
                            let snapshot = format!("{}:{}", mtime, meta.len());
                            let was_changed = job.watch_snapshot.get(&key) != Some(&snapshot);
                            if was_changed {
                                changed = true;
                            }
                            new_snap.insert(key, snapshot);
                        }
                    }
                    // Update snapshot in manager
                    if let Some(j) = mgr.get_mut(&job.id) {
                        j.watch_snapshot = new_snap;
                    }
                    if !changed && job.interval_ms > 0 {
                        // No changes detected, skip this trigger
                        mgr.mark_idle(&job.id);
                        continue;
                    }
                }

                // Check preflight command: if it fails, skip this run
                if let Some(ref cmd) = job.preflight_command {
                    debug!("Running preflight for job {}: {}", job.id, cmd);
                    let output = tokio::process::Command::new("sh")
                        .arg("-c")
                        .arg(cmd)
                        .output()
                        .await;
                    match output {
                        Ok(out) if !out.status.success() => {
                            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                            warn!("Preflight failed for job {}: {}", job.id, stderr);
                            mgr.mark_idle(&job.id);
                            continue;
                        }
                        Err(e) => {
                            warn!("Preflight command error for job {}: {}", job.id, e);
                            mgr.mark_idle(&job.id);
                            continue;
                        }
                        _ => {} // success, proceed
                    }
                }

                mgr.mark_running(&job.id);
                to_trigger.push(job.clone());
            }
            let _ = mgr.save();
        }

        // Trigger via callback
        let cb = match &self.trigger_cb {
            Some(cb) => cb.clone(),
            None => {
                warn!("No trigger callback set");
                return 0;
            }
        };

        for job in &to_trigger {
            debug!("Triggering loop job {} ({})", job.id, job.name);
            cb(job.clone()).await;
        }

        to_trigger.len()
    }

    // ── Event handlers ─────────────────────────────────────────────────────

    /// Handle a session idle event.
    pub async fn on_session_idle(&self) {
        self.maybe_run_due_jobs().await;
    }

    /// Force a specific job to run now.
    pub async fn trigger_job(&self, job_id: &str) {
        let job_opt = {
            let mut mgr = self.manager.write().await;
            if let Some(job) = mgr.get_mut(job_id) {
                // Make it due immediately
                job.last_run_at =
                    Some(chrono::Utc::now().timestamp_millis() - job.interval_ms as i64 - 1);
                job.status = LoopJobStatus::Idle;
                job.updated_at = chrono::Utc::now().timestamp_millis();
                Some(job.clone())
            } else {
                None
            }
        };

        if let Some(job) = job_opt {
            let cb = match &self.trigger_cb {
                Some(cb) => cb.clone(),
                None => return,
            };
            {
                let mut mgr = self.manager.write().await;
                mgr.mark_running(job_id);
                let _ = mgr.save();
            }
            cb(job).await;
        }
    }

    // ── Watchdog ───────────────────────────────────────────────────────────

    /// Start the watchdog loop (5s interval).
    pub fn start_watchdog(&self) {
        let mut guard = self.watchdog_handle.lock().unwrap();
        if guard.is_some() {
            return;
        }
        let manager = self.manager.clone();
        let run_lock = self.run_lock.clone();
        let shutdown = self.shutdown.clone();
        let trigger_cb = self.trigger_cb.clone();
        let timeout_ms = self.run_lock_timeout_ms;

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                if shutdown.load(Ordering::Acquire) {
                    break;
                }
                if run_lock.swap(true, Ordering::Acquire) {
                    continue;
                }
                let now = chrono::Utc::now().timestamp_millis();
                {
                    let mut mgr = manager.write().await;
                    mgr.recover_stale_runs(timeout_ms);
                }
                let due: Vec<LoopJob> = {
                    let mgr = manager.read().await;
                    mgr.due_jobs(now).iter().map(|j| (*j).clone()).collect()
                };
                if !due.is_empty() {
                    let mut to_trigger: Vec<LoopJob> = Vec::new();
                    {
                        let mut mgr = manager.write().await;
                        for job in &due {
                            if let Some(ref stop_file) = job.stop_file
                                && stop_file.exists()
                            {
                                mgr.mark_done(&job.id);
                                let _ = mgr.save();
                                continue;
                            }
                            if let Some(ref until) = job.until {
                                let p = crate::manager::loops_dir(std::path::Path::new("."))
                                    .join("until.txt");
                                if let Ok(content) = std::fs::read_to_string(p)
                                    && content.contains(until)
                                {
                                    mgr.mark_done(&job.id);
                                    let _ = mgr.save();
                                    continue;
                                }
                            }
                            if !job.watch_paths.is_empty() {
                                let mut changed = false;
                                let mut new_snap = std::collections::HashMap::new();
                                for path in &job.watch_paths {
                                    if let Ok(meta) = path.metadata() {
                                        let key = path.to_string_lossy().to_string();
                                        let mtime = meta
                                            .modified()
                                            .ok()
                                            .and_then(|t| t.elapsed().ok())
                                            .map(|d| d.as_secs())
                                            .unwrap_or(0);
                                        let sn = format!("{}:{}", mtime, meta.len());
                                        if job.watch_snapshot.get(&key) != Some(&sn) {
                                            changed = true;
                                        }
                                        new_snap.insert(key, sn);
                                    }
                                }
                                if let Some(j) = mgr.get_mut(&job.id) {
                                    j.watch_snapshot = new_snap;
                                }
                                if !changed && job.interval_ms > 0 {
                                    mgr.mark_idle(&job.id);
                                    continue;
                                }
                            }
                            mgr.mark_running(&job.id);
                            to_trigger.push(job.clone());
                        }
                        let _ = mgr.save();
                    }
                    if let Some(ref cb) = trigger_cb {
                        for job in &to_trigger {
                            debug!("[watchdog] Triggering loop job {}", job.id);
                            cb(job.clone()).await;
                        }
                    }
                }
                run_lock.store(false, Ordering::Release);
            }
        });
        *guard = Some(handle);
    }

    /// Stop the watchdog.
    pub fn stop_watchdog(&self) {
        self.shutdown.store(true, Ordering::Release);
        if let Ok(mut guard) = self.watchdog_handle.lock()
            && let Some(handle) = guard.take()
        {
            handle.abort();
        }
    }

    // ── Internal helpers ───────────────────────────────────────────────────

    fn cancel_timer(&self, job_id: &str) {
        if let Ok(mut timers) = self.job_timers.lock()
            && let Some(handle) = timers.remove(job_id)
        {
            handle.abort();
        }
    }

    fn spawn_job_timer(&self, job: &LoopJob) -> tokio::task::JoinHandle<()> {
        let interval = tokio::time::Duration::from_millis(job.interval_ms);
        let job_id = job.id.clone();
        let job_name = job.name.clone();
        let manager = self.manager.clone();
        let trigger_cb = self.trigger_cb.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;

                // Check job is still active
                let should_run = {
                    let mgr = manager.read().await;
                    mgr.get(&job_id)
                        .map(|j| j.status == LoopJobStatus::Idle && !j.max_runs_reached())
                        .unwrap_or(false)
                };
                if !should_run {
                    break;
                }

                // Mark running
                {
                    let mut mgr = manager.write().await;
                    mgr.mark_running(&job_id);
                    let _ = mgr.save();
                }

                // Trigger
                if let Some(ref cb) = trigger_cb
                    && let Some(job) = manager.read().await.get(&job_id).cloned()
                {
                    debug!("[timer] Triggering loop job {} ({})", job_id, job_name);
                    cb(job).await;
                }
            }
        })
    }
}

impl Drop for LoopScheduler {
    fn drop(&mut self) {
        self.stop_watchdog();
        if let Ok(mut timers) = self.job_timers.lock() {
            for (_, handle) in timers.drain() {
                handle.abort();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LoopJob;
    use std::sync::atomic::AtomicUsize;

    #[tokio::test]
    async fn test_empty_no_trigger() {
        let mgr = Arc::new(RwLock::new(LoopManager::new("test.json".into())));
        let sched = LoopScheduler::new(mgr);
        assert_eq!(sched.maybe_run_due_jobs().await, 0);
    }

    #[tokio::test]
    async fn test_maybe_run_due_jobs() {
        let mgr = Arc::new(RwLock::new(LoopManager::new("test.json".into())));
        let job = LoopJob::new_prompt("j1".into(), "t".into(), "a".into(), 0, true);
        mgr.write().await.create(job);

        let triggered = Arc::new(AtomicUsize::new(0));
        let tc = triggered.clone();
        let mut sched = LoopScheduler::new(mgr);
        sched.set_trigger_cb(Arc::new(move |_| {
            tc.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {})
        }));

        assert_eq!(sched.maybe_run_due_jobs().await, 1);
        assert_eq!(triggered.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_run_lock() {
        let mgr = Arc::new(RwLock::new(LoopManager::new("test.json".into())));
        let sched = LoopScheduler::new(mgr);
        sched.run_lock.store(true, Ordering::Release);
        assert_eq!(sched.maybe_run_due_jobs().await, 0);
    }

    #[tokio::test]
    async fn test_trigger_job() {
        let mgr = Arc::new(RwLock::new(LoopManager::new("test.json".into())));
        let job = LoopJob::new_prompt("j1".into(), "t".into(), "a".into(), 300_000, false);
        mgr.write().await.create(job);

        let triggered = Arc::new(AtomicUsize::new(0));
        let tc = triggered.clone();
        let mut sched = LoopScheduler::new(mgr);
        sched.set_trigger_cb(Arc::new(move |_| {
            tc.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {})
        }));

        sched.trigger_job("j1").await;
        assert_eq!(triggered.load(Ordering::SeqCst), 1);
    }
}
