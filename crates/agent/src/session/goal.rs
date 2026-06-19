use std::sync::RwLock;
use std::sync::atomic::{AtomicU32, Ordering};

use super::entries::GoalEntry;

pub struct GoalManager {
    goal: RwLock<Option<GoalEntry>>,
    continuation_count: AtomicU32,
    max_continuations: u32,
}

impl GoalManager {
    pub fn new() -> Self {
        Self {
            goal: RwLock::new(None),
            continuation_count: AtomicU32::new(0),
            max_continuations: 5,
        }
    }

    pub fn with_max_continuations(mut self, max: u32) -> Self {
        self.max_continuations = max;
        self
    }

    pub fn load(&self, entry: GoalEntry) {
        if let Ok(mut g) = self.goal.write() {
            *g = Some(entry);
            self.continuation_count.store(0, Ordering::Relaxed);
        }
    }

    pub fn get(&self) -> Option<GoalEntry> {
        self.goal.read().ok()?.clone()
    }

    pub fn create(
        &self,
        objective: String,
        token_budget: Option<i64>,
    ) -> Result<GoalEntry, String> {
        let mut guard = self.goal.write().map_err(|e| e.to_string())?;
        if guard.is_some() {
            return Err("cannot create a new goal because this thread already has a goal; use update_goal only when the existing goal is complete".to_string());
        }
        let now = chrono::Utc::now().timestamp_millis();
        let entry = GoalEntry {
            objective,
            status: "active".to_string(),
            token_budget,
            tokens_used: 0,
            time_used_seconds: 0,
            created_at: now,
            updated_at: now,
        };
        *guard = Some(entry.clone());
        Ok(entry)
    }

    pub fn update_status(&self, status: String) -> Result<GoalEntry, String> {
        let mut guard = self.goal.write().map_err(|e| e.to_string())?;
        let entry = guard
            .as_mut()
            .ok_or_else(|| "no active goal to update".to_string())?;
        let allowed = match status.as_str() {
            "complete" | "blocked" | "budget_limited" => true,
            _ => false,
        };
        if !allowed {
            return Err(
                "update_goal can only mark the existing goal complete, blocked, or budget_limited"
                    .to_string(),
            );
        }
        entry.status = status;
        entry.updated_at = chrono::Utc::now().timestamp_millis();
        Ok(entry.clone())
    }

    pub fn add_token_usage(&self, tokens: i64) -> Option<GoalEntry> {
        if let Ok(mut guard) = self.goal.write()
            && let Some(ref mut entry) = *guard {
                entry.tokens_used += tokens;
                entry.updated_at = chrono::Utc::now().timestamp_millis();

                if let Some(budget) = entry.token_budget
                    && entry.tokens_used >= budget && entry.status == "active" {
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
        Ok(())
    }

    pub fn set_paused(&self) -> Result<GoalEntry, String> {
        let mut guard = self.goal.write().map_err(|e| e.to_string())?;
        let entry = guard
            .as_mut()
            .ok_or_else(|| "no goal to pause".to_string())?;
        if entry.status != "active" && entry.status != "budget_limited" {
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
        Ok(entry.clone())
    }

    pub fn set_objective(&self, objective: String) -> Result<GoalEntry, String> {
        let mut guard = self.goal.write().map_err(|e| e.to_string())?;
        let entry = guard
            .as_mut()
            .ok_or_else(|| "no goal to edit".to_string())?;
        entry.objective = objective;
        entry.updated_at = chrono::Utc::now().timestamp_millis();
        Ok(entry.clone())
    }

    pub fn can_continue(&self) -> bool {
        let count = self.continuation_count.load(Ordering::Relaxed);
        if count >= self.max_continuations {
            return false;
        }
        self.goal
            .read()
            .ok()
            .and_then(|g| g.as_ref().map(|e| e.status == "active"))
            .unwrap_or(false)
    }

    pub fn register_continuation(&self) -> bool {
        let prev = self.continuation_count.fetch_add(1, Ordering::Relaxed);
        (prev + 1) <= self.max_continuations
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

    pub fn status(&self) -> Option<String> {
        self.goal.read().ok()?.as_ref().map(|e| e.status.clone())
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
            .field("max_continuations", &self.max_continuations)
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
        let goal = gm.create("test objective".into(), None).unwrap();
        assert_eq!(goal.objective, "test objective");
        assert_eq!(goal.status, "active");
        assert_eq!(goal.tokens_used, 0);
        assert!(gm.get().is_some());
    }

    #[test]
    fn test_create_fails_if_exists() {
        let gm = GoalManager::new();
        gm.create("first".into(), None).unwrap();
        let err = gm.create("second".into(), None).unwrap_err();
        assert!(err.contains("already has a goal"));
    }

    #[test]
    fn test_update_status() {
        let gm = GoalManager::new();
        gm.create("test".into(), None).unwrap();
        let g = gm.update_status("complete".into()).unwrap();
        assert_eq!(g.status, "complete");
        let err = gm.update_status("invalid".into()).unwrap_err();
        assert!(err.contains("can only mark"));
    }

    #[test]
    fn test_clear() {
        let gm = GoalManager::new();
        gm.create("test".into(), None).unwrap();
        assert!(gm.get().is_some());
        gm.clear().unwrap();
        assert!(gm.get().is_none());
    }

    #[test]
    fn test_pause_resume() {
        let gm = GoalManager::new();
        gm.create("test".into(), None).unwrap();
        let g = gm.set_paused().unwrap();
        assert_eq!(g.status, "paused");
        let g = gm.set_active().unwrap();
        assert_eq!(g.status, "active");
    }

    #[test]
    fn test_pause_fails_if_not_active() {
        let gm = GoalManager::new();
        gm.create("test".into(), None).unwrap();
        gm.update_status("complete".into()).unwrap();
        let err = gm.set_paused().unwrap_err();
        assert!(err.contains("cannot pause"));
    }

    #[test]
    fn test_set_objective() {
        let gm = GoalManager::new();
        gm.create("old".into(), None).unwrap();
        let g = gm.set_objective("new".into()).unwrap();
        assert_eq!(g.objective, "new");
    }

    #[test]
    fn test_token_budget_exhausted() {
        let gm = GoalManager::new();
        gm.create("test".into(), Some(100)).unwrap();
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
        gm.create("test".into(), Some(100)).unwrap();
        assert_eq!(gm.remaining_tokens(), Some(100));
        gm.add_token_usage(30);
        assert_eq!(gm.remaining_tokens(), Some(70));
        gm.add_token_usage(80);
        assert_eq!(gm.remaining_tokens(), Some(0));
    }

    #[test]
    fn test_continuation() {
        let gm = GoalManager::new();
        gm.create("test".into(), None).unwrap();
        assert!(gm.can_continue());
        assert!(gm.register_continuation());
        assert!(gm.can_continue());
        gm.register_continuation();
        gm.register_continuation();
        gm.register_continuation();
        assert!(gm.can_continue());
        gm.register_continuation(); // 5th — reaches max (default 5)
        assert!(!gm.can_continue());
        assert!(!gm.register_continuation());
        gm.reset_continuation_count();
        assert!(gm.can_continue());
    }

    #[test]
    fn test_cannot_continue_if_paused() {
        let gm = GoalManager::new();
        gm.create("test".into(), None).unwrap();
        gm.set_paused().unwrap();
        assert!(!gm.can_continue());
    }

    #[test]
    fn test_cannot_continue_if_budget_exhausted() {
        let gm = GoalManager::new();
        gm.create("test".into(), Some(10)).unwrap();
        gm.add_token_usage(10);
        assert!(!gm.can_continue());
    }

    #[test]
    fn test_load_restores_state() {
        let gm = GoalManager::new();
        let entry = GoalEntry {
            objective: "loaded".into(),
            status: "paused".into(),
            token_budget: Some(500),
            tokens_used: 100,
            time_used_seconds: 60,
            created_at: 1000,
            updated_at: 2000,
        };
        gm.load(entry);
        let g = gm.get().unwrap();
        assert_eq!(g.objective, "loaded");
        assert_eq!(g.status, "paused");
        assert_eq!(g.token_budget, Some(500));
        assert_eq!(gm.remaining_tokens(), Some(400));
    }

    #[test]
    fn test_with_max_continuations() {
        let gm = GoalManager::with_max_continuations(GoalManager::new(), 2);
        gm.create("test".into(), None).unwrap();
        assert!(gm.register_continuation());
        assert!(gm.register_continuation());
        assert!(!gm.register_continuation());
    }
}
