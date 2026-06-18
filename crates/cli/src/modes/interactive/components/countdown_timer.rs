//! Reusable countdown timer for dialog components


use std::time::Instant;

/// A countdown timer that tracks remaining time
pub struct CountdownTimer {
    start: Instant,
    timeout_ms: u64,
    expired: bool,
}

impl CountdownTimer {
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            start: Instant::now(),
            timeout_ms,
            expired: false,
        }
    }

    /// Get remaining seconds (rounded up)
    pub fn remaining_seconds(&self) -> u64 {
        let elapsed = self.start.elapsed().as_millis() as u64;
        if elapsed >= self.timeout_ms {
            0
        } else {
            let remaining = self.timeout_ms - elapsed;
            (remaining + 999) / 1000
        }
    }

    /// Check if the timer has expired
    pub fn is_expired(&self) -> bool {
        self.expired || self.start.elapsed().as_millis() as u64 >= self.timeout_ms
    }

    /// Mark the timer as expired
    pub fn expire(&mut self) {
        self.expired = true;
    }
}
