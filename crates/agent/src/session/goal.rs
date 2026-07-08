use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use super::entries::GoalEntry;

/// Maximum length of a goal objective string.
pub const MAX_GOAL_OBJECTIVE_CHARS: usize = 1000;

/// Maximum length of a goal completion criterion string.
pub const MAX_GOAL_CRITERION_CHARS: usize = 2000;

/// Maximum number of automatic auto-continuation turns before usage_limited.
pub const DEFAULT_MAX_CONTINUATIONS: u32 = 50;

pub struct GoalManager {
    goal: RwLock<Option<GoalEntry>>,
    continuation_count: AtomicU32,
    objective_updated: AtomicBool,
    /// Shared flag between steering and stop-after-turn closures to prevent
    /// injecting the budget-limit steering message more than once.
    budget_limit_reported: AtomicBool,
}

impl GoalManager {
    pub fn new() -> Self {
        Self {
            goal: RwLock::new(None),
            continuation_count: AtomicU32::new(0),
            objective_updated: AtomicBool::new(false),
            budget_limit_reported: AtomicBool::new(false),
        }
    }

    pub fn load(&self, entry: GoalEntry) {
        if let Ok(mut g) = self.goal.write() {
            *g = Some(entry);
            self.continuation_count.store(0, Ordering::Relaxed);
            self.budget_limit_reported.store(false, Ordering::Relaxed);
        }
    }

    pub fn get(&self) -> Option<GoalEntry> {
        self.goal.read().ok()?.clone()
    }

    pub fn create(
        &self,
        objective: String,
        completion_criterion: String,
        token_budget: Option<i64>,
        max_turns: Option<u32>,
    ) -> Result<GoalEntry, String> {
        if objective.chars().count() > MAX_GOAL_OBJECTIVE_CHARS {
            return Err(format!(
                "goal objective too long ({} chars, max {})",
                objective.chars().count(),
                MAX_GOAL_OBJECTIVE_CHARS
            ));
        }
        if completion_criterion.chars().count() > MAX_GOAL_CRITERION_CHARS {
            return Err(format!(
                "completion criterion too long ({} chars, max {})",
                completion_criterion.chars().count(),
                MAX_GOAL_CRITERION_CHARS
            ));
        }
        let mut guard = self.goal.write().map_err(|e| e.to_string())?;
        if guard.is_some() {
            return Err(
                "cannot create a new goal because this thread already has a goal".to_string(),
            );
        }
        let now = chrono::Utc::now().timestamp_millis();
        let entry = GoalEntry {
            objective,
            completion_criterion,
            status: "active".to_string(),
            token_budget,
            max_turns,
            tokens_used: 0,
            time_used_seconds: 0,
            created_at: now,
            updated_at: now,
        };
        *guard = Some(entry.clone());
        self.budget_limit_reported.store(false, Ordering::Relaxed);
        Ok(entry)
    }

    pub fn update_status(&self, status: String) -> Result<GoalEntry, String> {
        let mut guard = self.goal.write().map_err(|e| e.to_string())?;
        let entry = guard
            .as_mut()
            .ok_or_else(|| "no active goal to update".to_string())?;
        let allowed = match status.as_str() {
            "complete" | "blocked" | "budget_limited" | "usage_limited" => true,
            _ => false,
        };
        if !allowed {
            return Err(
                "goal(op:\"complete\") or goal(op:\"blocked\") can only mark the existing goal complete or blocked"
                    .to_string(),
            );
        }
        entry.status = status;
        entry.updated_at = chrono::Utc::now().timestamp_millis();
        self.budget_limit_reported.store(false, Ordering::Relaxed);
        Ok(entry.clone())
    }

    pub fn add_token_usage(&self, tokens: i64) -> Option<GoalEntry> {
        if let Ok(mut guard) = self.goal.write()
            && let Some(ref mut entry) = *guard
        {
            entry.tokens_used += tokens;
            entry.updated_at = chrono::Utc::now().timestamp_millis();

            if let Some(budget) = entry.token_budget
                && entry.tokens_used >= budget
                && entry.status == "active"
            {
                entry.status = "budget_limited".to_string();
                self.continuation_count.store(0, Ordering::Relaxed);
            }
            return Some(entry.clone());
        }
        None
    }

    pub fn remaining_tokens(&self) -> Option<i64> {
        self.goal
            .read()
            .ok()?
            .as_ref()
            .and_then(|g| g.token_budget.map(|budget| (budget - g.tokens_used).max(0)))
    }

    pub fn clear(&self) -> Result<(), String> {
        let mut guard = self.goal.write().map_err(|e| e.to_string())?;
        *guard = None;
        self.continuation_count.store(0, Ordering::Relaxed);
        self.budget_limit_reported.store(false, Ordering::Relaxed);
        Ok(())
    }

    pub fn set_paused(&self) -> Result<GoalEntry, String> {
        let mut guard = self.goal.write().map_err(|e| e.to_string())?;
        let entry = guard
            .as_mut()
            .ok_or_else(|| "no goal to pause".to_string())?;
        if entry.status != "active"
            && entry.status != "budget_limited"
            && entry.status != "usage_limited"
        {
            return Err(format!("cannot pause goal with status '{}'", entry.status));
        }
        entry.status = "paused".to_string();
        entry.updated_at = chrono::Utc::now().timestamp_millis();
        self.continuation_count.store(0, Ordering::Relaxed);
        Ok(entry.clone())
    }

    pub fn set_active(&self) -> Result<GoalEntry, String> {
        let mut guard = self.goal.write().map_err(|e| e.to_string())?;
        let entry = guard
            .as_mut()
            .ok_or_else(|| "no goal to resume".to_string())?;
        if entry.status != "paused" && entry.status != "blocked" {
            return Err(format!("cannot resume goal with status '{}'", entry.status));
        }
        entry.status = "active".to_string();
        entry.updated_at = chrono::Utc::now().timestamp_millis();
        self.continuation_count.store(0, Ordering::Relaxed);
        self.budget_limit_reported.store(false, Ordering::Relaxed);
        Ok(entry.clone())
    }

    pub fn set_objective(&self, objective: String) -> Result<GoalEntry, String> {
        if objective.chars().count() > MAX_GOAL_OBJECTIVE_CHARS {
            return Err(format!(
                "goal objective too long ({} chars, max {})",
                objective.chars().count(),
                MAX_GOAL_OBJECTIVE_CHARS
            ));
        }
        let mut guard = self.goal.write().map_err(|e| e.to_string())?;
        let entry = guard
            .as_mut()
            .ok_or_else(|| "no goal to edit".to_string())?;
        entry.objective = objective;
        entry.updated_at = chrono::Utc::now().timestamp_millis();
        // Auto-restore terminal states (complete, budget_limited, usage_limited) to active
        // when the user explicitly edits the objective — they clearly want to keep working.
        if matches!(
            entry.status.as_str(),
            "complete" | "budget_limited" | "usage_limited"
        ) {
            entry.status = "active".to_string();
            self.continuation_count.store(0, Ordering::Relaxed);
            self.budget_limit_reported.store(false, Ordering::Relaxed);
        }
        self.objective_updated.store(true, Ordering::Relaxed);
        Ok(entry.clone())
    }

    /// Mark that the goal objective has been updated externally.
    /// This flag signals the steering closure to inject objective_updated.md.
    pub fn mark_objective_updated(&self) {
        self.objective_updated.store(true, Ordering::Relaxed);
    }

    /// Atomically check and clear the objective-updated flag.
    pub fn take_objective_updated(&self) -> bool {
        self.objective_updated.swap(false, Ordering::Relaxed)
    }

    pub fn pause_on_interrupt(&self) -> Result<GoalEntry, String> {
        let mut guard = self.goal.write().map_err(|e| e.to_string())?;
        let entry = guard
            .as_mut()
            .ok_or_else(|| "no goal to pause".to_string())?;
        if entry.status != "active"
            && entry.status != "budget_limited"
            && entry.status != "usage_limited"
        {
            return Err(format!("cannot pause goal with status '{}'", entry.status));
        }
        entry.status = "paused".to_string();
        entry.updated_at = chrono::Utc::now().timestamp_millis();
        self.continuation_count.store(0, Ordering::Relaxed);
        Ok(entry.clone())
    }

    pub fn can_continue(&self) -> bool {
        self.goal
            .read()
            .ok()
            .and_then(|g| g.as_ref().map(|e| e.status == "active"))
            .unwrap_or(false)
    }

    pub fn register_continuation(&self) -> Result<(), String> {
        let count = self.continuation_count.fetch_add(1, Ordering::Relaxed) + 1;
        // Check max_turns limit and auto-transition to usage_limited
        if let Ok(guard) = self.goal.read()
            && let Some(ref entry) = *guard
            && let Some(max) = entry.max_turns
            && count >= max
        {
            drop(guard);
            self.update_status("usage_limited".to_string())?;
            self.budget_limit_reported.store(false, Ordering::Relaxed);
            return Err(format!(
                "goal continuation limit reached ({}/{} turns)",
                count, max
            ));
        }
        Ok(())
    }

    pub fn reset_continuation_count(&self) {
        self.continuation_count.store(0, Ordering::Relaxed);
    }

    pub fn is_budget_exhausted(&self) -> bool {
        self.goal
            .read()
            .ok()
            .and_then(|g| {
                g.as_ref()
                    .map(|e| e.token_budget.map(|b| e.tokens_used >= b).unwrap_or(false))
            })
            .unwrap_or(false)
    }

    /// Add wall-clock seconds to the goal's time tracking.
    /// Returns the updated goal entry if one exists.
    pub fn add_time_usage(&self, elapsed_secs: i64) -> Option<GoalEntry> {
        if let Ok(mut guard) = self.goal.write()
            && let Some(ref mut entry) = *guard
        {
            entry.time_used_seconds = entry.time_used_seconds.saturating_add(elapsed_secs);
            entry.updated_at = chrono::Utc::now().timestamp_millis();
            return Some(entry.clone());
        }
        None
    }

    /// Check whether the budget-limit steering message has already been injected.
    pub fn budget_limit_reported(&self) -> bool {
        self.budget_limit_reported.load(Ordering::Relaxed)
    }

    /// Mark the budget-limit steering message as having been injected.
    /// Returns the previous value (true = already reported).
    pub fn mark_budget_limit_reported(&self) -> bool {
        self.budget_limit_reported.swap(true, Ordering::Relaxed)
    }

    pub fn status(&self) -> Option<String> {
        self.goal.read().ok()?.as_ref().map(|e| e.status.clone())
    }

    pub fn get_continuation_count(&self) -> u32 {
        self.continuation_count.load(Ordering::Relaxed)
    }
}

impl Default for GoalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for GoalManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoalManager")
            .field("goal", &self.goal)
            .field(
                "continuation_count",
                &self.continuation_count.load(Ordering::Relaxed),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get() {
        let gm = GoalManager::new();
        assert!(gm.get().is_none());
        let goal = gm
            .create("test objective".into(), "test criterion".into(), None, None)
            .unwrap();
        assert_eq!(goal.objective, "test objective");
        assert_eq!(goal.completion_criterion, "test criterion");
        assert_eq!(goal.status, "active");
        assert_eq!(goal.tokens_used, 0);
        assert_eq!(goal.max_turns, None);
        assert!(gm.get().is_some());
    }

    #[test]
    fn test_create_fails_if_exists() {
        let gm = GoalManager::new();
        gm.create("first".into(), "criterion".into(), None, None)
            .unwrap();
        let err = gm
            .create("second".into(), "criterion".into(), None, None)
            .unwrap_err();
        assert!(err.contains("already has a goal"));
    }

    #[test]
    fn test_update_status() {
        let gm = GoalManager::new();
        gm.create("test".into(), "criterion".into(), None, None)
            .unwrap();
        let g = gm.update_status("complete".into()).unwrap();
        assert_eq!(g.status, "complete");
        let err = gm.update_status("invalid".into()).unwrap_err();
        assert!(err.contains("can only mark"));
    }

    #[test]
    fn test_clear() {
        let gm = GoalManager::new();
        gm.create("test".into(), "criterion".into(), None, None)
            .unwrap();
        assert!(gm.get().is_some());
        gm.clear().unwrap();
        assert!(gm.get().is_none());
    }

    #[test]
    fn test_pause_resume() {
        let gm = GoalManager::new();
        gm.create("test".into(), "criterion".into(), None, None)
            .unwrap();
        let g = gm.set_paused().unwrap();
        assert_eq!(g.status, "paused");
        let g = gm.set_active().unwrap();
        assert_eq!(g.status, "active");
    }

    #[test]
    fn test_pause_fails_if_not_active() {
        let gm = GoalManager::new();
        gm.create("test".into(), "criterion".into(), None, None)
            .unwrap();
        gm.update_status("complete".into()).unwrap();
        let err = gm.set_paused().unwrap_err();
        assert!(err.contains("cannot pause"));
    }

    #[test]
    fn test_set_objective_auto_restores_terminal_states() {
        let gm = GoalManager::new();
        gm.create("old".into(), "criterion".into(), None, None)
            .unwrap();
        let g = gm.set_objective("new".into()).unwrap();
        assert_eq!(g.objective, "new");

        // complete → active edit
        gm.update_status("complete".into()).unwrap();
        let g = gm.set_objective("revised".into()).unwrap();
        assert_eq!(g.objective, "revised");
        assert_eq!(g.status, "active");

        // Go back to complete, then budget_limited → active edit
        gm.update_status("complete".into()).unwrap();
        // Simulate a budget-limited goal by creating a fresh one
        gm.clear().unwrap();
        gm.create("fresh".into(), "criterion".into(), Some(100), None)
            .unwrap();
        gm.add_token_usage(100);
        assert_eq!(gm.get().unwrap().status, "budget_limited");
        gm.set_objective("budget revised".into()).unwrap();
        assert_eq!(gm.get().unwrap().status, "active");
    }

    #[test]
    fn test_token_budget_exhausted() {
        let gm = GoalManager::new();
        gm.create("test".into(), "criterion".into(), Some(100), None)
            .unwrap();
        assert!(!gm.is_budget_exhausted());
        gm.add_token_usage(60);
        assert!(!gm.is_budget_exhausted());
        let updated = gm.add_token_usage(50);
        assert!(gm.is_budget_exhausted());
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().status, "budget_limited");
    }

    #[test]
    fn test_remaining_tokens() {
        let gm = GoalManager::new();
        gm.create("test".into(), "criterion".into(), Some(100), None)
            .unwrap();
        assert_eq!(gm.remaining_tokens(), Some(100));
        gm.add_token_usage(30);
        assert_eq!(gm.remaining_tokens(), Some(70));
        gm.add_token_usage(80);
        assert_eq!(gm.remaining_tokens(), Some(0));
    }

    #[test]
    fn test_continuation() {
        let gm = GoalManager::new();
        gm.create("test".into(), "criterion".into(), None, None)
            .unwrap();
        assert!(gm.can_continue());
        gm.register_continuation().ok();
        assert!(gm.can_continue());
        gm.register_continuation().ok();
        gm.register_continuation().ok();
        gm.register_continuation().ok();
        assert!(gm.can_continue());
        gm.register_continuation().ok(); // unlimited
        assert!(gm.can_continue());
        gm.reset_continuation_count();
        assert!(gm.can_continue());
    }

    #[test]
    fn test_continuation_with_max_turns() {
        let gm = GoalManager::new();
        gm.create("test".into(), "criterion".into(), None, Some(3))
            .unwrap();
        assert!(gm.can_continue());
        assert!(gm.register_continuation().is_ok());
        assert!(gm.can_continue());
        assert!(gm.register_continuation().is_ok());
        assert!(gm.can_continue());
        assert!(gm.register_continuation().is_ok()); // 3rd = limit hit
        assert!(!gm.can_continue());
        assert_eq!(gm.get().unwrap().status, "usage_limited");
    }

    #[test]
    fn test_cannot_continue_if_paused() {
        let gm = GoalManager::new();
        gm.create("test".into(), "criterion".into(), None, None)
            .unwrap();
        gm.set_paused().unwrap();
        assert!(!gm.can_continue());
    }

    #[test]
    fn test_cannot_continue_if_budget_exhausted() {
        let gm = GoalManager::new();
        gm.create("test".into(), "criterion".into(), Some(10), None)
            .unwrap();
        gm.add_token_usage(10);
        assert!(!gm.can_continue());
    }

    #[test]
    fn test_cannot_continue_if_usage_limited() {
        let gm = GoalManager::new();
        gm.create("test".into(), "criterion".into(), None, Some(1))
            .unwrap();
        gm.register_continuation().ok();
        assert!(!gm.can_continue());
    }

    #[test]
    fn test_load_restores_state() {
        let gm = GoalManager::new();
        let entry = GoalEntry {
            objective: "loaded".into(),
            completion_criterion: "criterion".into(),
            status: "paused".into(),
            token_budget: Some(500),
            max_turns: Some(10),
            tokens_used: 100,
            time_used_seconds: 60,
            created_at: 1000,
            updated_at: 2000,
        };
        gm.load(entry);
        let g = gm.get().unwrap();
        assert_eq!(g.objective, "loaded");
        assert_eq!(g.completion_criterion, "criterion");
        assert_eq!(g.status, "paused");
        assert_eq!(g.token_budget, Some(500));
        assert_eq!(g.max_turns, Some(10));
        assert_eq!(gm.remaining_tokens(), Some(400));
    }

    #[test]
    fn test_pause_on_interrupt() {
        let gm = GoalManager::new();
        gm.create("test".into(), "criterion".into(), None, None)
            .unwrap();
        let g = gm.pause_on_interrupt().unwrap();
        assert_eq!(g.status, "paused");
        let err = gm.pause_on_interrupt().unwrap_err();
        assert!(err.contains("cannot pause"));
    }

    #[test]
    fn test_add_time_usage() {
        let gm = GoalManager::new();
        gm.create("test".into(), "criterion".into(), None, None)
            .unwrap();
        assert_eq!(gm.get().unwrap().time_used_seconds, 0);
        gm.add_time_usage(120);
        assert_eq!(gm.get().unwrap().time_used_seconds, 120);
        gm.add_time_usage(30);
        assert_eq!(gm.get().unwrap().time_used_seconds, 150);
    }

    #[test]
    fn test_budget_limit_reported_flag() {
        let gm = GoalManager::new();
        assert!(!gm.budget_limit_reported());
        assert!(!gm.mark_budget_limit_reported()); // false = not yet reported
        assert!(gm.budget_limit_reported());
        assert!(gm.mark_budget_limit_reported()); // true = was already reported
    }
}
