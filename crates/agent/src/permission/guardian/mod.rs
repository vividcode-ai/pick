use std::sync::atomic::{AtomicU32, Ordering};

const MAX_CONSECUTIVE_DENIALS: u32 = 3;
const MAX_RECENT_DENIALS: u32 = 10;
const RECENT_WINDOW_SIZE: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GuardianAction {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Default)]
pub struct GuardianConfig {
    pub enabled: bool,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub strict_auto_review: bool,
}

pub struct Guardian {
    config: GuardianConfig,
    consecutive_denials: AtomicU32,
    recent_denials: std::sync::Mutex<Vec<bool>>,
    denial_count: AtomicU32,
}

impl Guardian {
    pub fn new(config: GuardianConfig) -> Self {
        Self {
            config,
            consecutive_denials: AtomicU32::new(0),
            recent_denials: std::sync::Mutex::new(Vec::with_capacity(RECENT_WINDOW_SIZE)),
            denial_count: AtomicU32::new(0),
        }
    }

    pub fn config(&self) -> &GuardianConfig {
        &self.config
    }

    pub fn is_circuit_broken(&self) -> bool {
        if self.consecutive_denials.load(Ordering::SeqCst) >= MAX_CONSECUTIVE_DENIALS {
            return true;
        }
        if self.denial_count.load(Ordering::SeqCst) >= MAX_RECENT_DENIALS {
            return true;
        }
        false
    }

    pub fn record_result(&self, action: GuardianAction) {
        match action {
            GuardianAction::Allow => {
                self.consecutive_denials.store(0, Ordering::SeqCst);
            }
            GuardianAction::Deny => {
                self.consecutive_denials.fetch_add(1, Ordering::SeqCst);
                self.denial_count.fetch_add(1, Ordering::SeqCst);

                let mut recent = self.recent_denials.lock().unwrap();
                recent.push(true);
                if recent.len() > RECENT_WINDOW_SIZE {
                    let removed = recent.remove(0);
                    if removed {
                        self.denial_count.fetch_sub(1, Ordering::SeqCst);
                    }
                }
            }
        }
    }

    pub fn circuit_breaker_message(&self) -> Option<&'static str> {
        if self.is_circuit_broken() {
            Some(
                "[Guardian] Circuit breaker triggered: too many actions were denied. \
                 The current turn has been interrupted. Please reconsider your approach.",
            )
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guardian_default_not_broken() {
        let guardian = Guardian::new(GuardianConfig::default());
        assert!(!guardian.is_circuit_broken());
    }

    #[test]
    fn test_guardian_circuit_breaker_consecutive() {
        let guardian = Guardian::new(GuardianConfig::default());
        for _ in 0..3 {
            assert!(!guardian.is_circuit_broken());
            guardian.record_result(GuardianAction::Deny);
        }
        assert!(guardian.is_circuit_broken());
    }

    #[test]
    fn test_guardian_allow_resets_consecutive() {
        let guardian = Guardian::new(GuardianConfig::default());
        guardian.record_result(GuardianAction::Deny);
        guardian.record_result(GuardianAction::Deny);
        guardian.record_result(GuardianAction::Allow);
        assert!(!guardian.is_circuit_broken());
    }

    #[test]
    fn test_guardian_message_when_broken() {
        let guardian = Guardian::new(GuardianConfig::default());
        for _ in 0..3 {
            guardian.record_result(GuardianAction::Deny);
        }
        assert!(guardian.circuit_breaker_message().is_some());
    }
}
