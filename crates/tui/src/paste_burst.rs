//! Timing-based paste burst detection with retroactive capture.
//! On Windows, crossterm never emits `Event::Paste(String)` — pasted text
//! arrives as individual `Event::Key(Char(c))` events in rapid succession.
//! This module detects such bursts by tracking inter-character timing.
//!
//! Design:
//! - Characters within CHAR_INTERVAL_MS are considered potential paste.
//! - First 2 chars pass through (inserted normally via handle_key).
//! - On the 3rd fast char, a burst is confirmed → retroactively capture the
//!   already-inserted chars and start buffering subsequent ones.
//! - On idle timeout or non-char input, the buffer is flushed as a paste.
//! - The small interval (12ms) ensures normal typing (200ms+) is never caught.

use std::time::Instant;

/// Max interval (ms) between chars to be considered part of a paste burst.
/// Windows delivers pasted chars via ReadConsoleInputW at 15-30ms intervals,
/// so 25ms reliably catches pastes while staying below normal typing (~150ms).
/// On Unix, the raw byte stream delivers paste events much faster (sub-ms).
#[cfg(windows)]
const CHAR_INTERVAL_MS: u128 = 25;
#[cfg(not(windows))]
const CHAR_INTERVAL_MS: u128 = 10;

/// Idle timeout (ms) after which a buffered burst is flushed.
/// Windows needs a slightly longer timeout due to event delivery latency.
#[cfg(windows)]
const IDLE_TIMEOUT_MS: u128 = 60;
#[cfg(not(windows))]
const IDLE_TIMEOUT_MS: u128 = 30;

/// Minimum consecutive fast chars before confirming a paste burst.
const MIN_BURST: u16 = 3;

/// Number of passthrough chars to retroactively capture when burst is confirmed.
const RETRO_COUNT: usize = (MIN_BURST - 1) as usize; // 2

/// Action returned by `PasteBurst::push_char` telling the event loop what to do.
#[derive(Debug)]
pub enum CharAction {
    /// This char is NOT part of a paste burst; process normally via handle_key.
    Passthrough,
    /// This char is being accumulated in the burst buffer; do NOT insert.
    Buffering,
    /// A paste burst was confirmed — retroactively capture the last N chars
    /// and insert the full batch as a paste.
    /// The event loop should call `editor.delete_last_chars(retro_count)` then
    /// `handle_paste(&text)`.
    RetroFlush { text: String, retro_count: usize },
    /// A paste burst completed (idle timeout or non-char input);
    /// insert the buffered text via handle_paste.
    Flush(String),
}

#[derive(Debug, Clone, PartialEq)]
enum PasteBurstState {
    /// No ongoing paste detection.
    Idle,
    /// First char passed through; watching for a second within the interval.
    Watching,
    /// Paste burst in progress; accumulating chars until idle timeout.
    Buffering,
}

/// Detects paste bursts by monitoring `KeyCode::Char` timing.
pub struct PasteBurst {
    state: PasteBurstState,
    /// Count of consecutive chars within CHAR_INTERVAL_MS.
    consecutive_count: u16,
    /// Timestamp of the last char processed.
    last_char_time: Option<Instant>,
    /// Characters that were passthrough'd and may need retroactive capture.
    /// Stores at most RETRO_COUNT chars.
    passthrough_chars: String,
    /// Characters accumulated during Buffering state.
    buffer: String,
}

impl PasteBurst {
    pub fn new() -> Self {
        Self {
            state: PasteBurstState::Idle,
            consecutive_count: 0,
            last_char_time: None,
            passthrough_chars: String::with_capacity(RETRO_COUNT * 4),
            buffer: String::new(),
        }
    }

    /// Feed a `KeyCode::Char(c)` event with its timestamp.
    ///
    /// Returns:
    /// - `Passthrough` — process the char as a normal key press (first 1-2 chars).
    /// - `Buffering` — char is being accumulated; do NOT insert.
    /// - `RetroFlush{text, retro_count}` — burst confirmed. Undo N chars via
    ///   `editor.delete_last_chars(retro_count)` then `handle_paste(&text)`.
    /// - `Flush(text)` — buffered chars ready as paste (via handle_paste).
    pub fn push_char(&mut self, c: char, now: Instant) -> CharAction {
        let gap = self
            .last_char_time
            .map(|t| now.duration_since(t).as_millis());

        match &self.state {
            PasteBurstState::Idle => {
                self.state = PasteBurstState::Watching;
                self.consecutive_count = 1;
                self.last_char_time = Some(now);
                self.passthrough_chars.clear();
                self.passthrough_chars.push(c);
                CharAction::Passthrough
            }
            PasteBurstState::Watching => {
                let is_fast = gap.map_or(false, |g| g < CHAR_INTERVAL_MS);
                if is_fast {
                    self.consecutive_count += 1;
                    self.last_char_time = Some(now);

                    if self.consecutive_count >= MIN_BURST {
                        // Burst confirmed! Retroactively capture passthrough chars.
                        let mut text = std::mem::take(&mut self.passthrough_chars);
                        text.push(c);

                        self.state = PasteBurstState::Buffering;
                        self.buffer.clear();
                        CharAction::RetroFlush {
                            text,
                            retro_count: RETRO_COUNT,
                        }
                    } else {
                        // Still watching — save for potential retro capture.
                        self.passthrough_chars.push(c);
                        CharAction::Passthrough
                    }
                } else {
                    // Gap too long → not a burst. Reset watch.
                    self.consecutive_count = 1;
                    self.last_char_time = Some(now);
                    self.passthrough_chars.clear();
                    self.passthrough_chars.push(c);
                    CharAction::Passthrough
                }
            }
            PasteBurstState::Buffering => {
                let is_fast = gap.map_or(false, |g| g < CHAR_INTERVAL_MS);
                if is_fast {
                    self.buffer.push(c);
                    self.last_char_time = Some(now);
                    CharAction::Buffering
                } else {
                    // Gap too long; flush buffer and restart watch for this char.
                    let pasted = std::mem::take(&mut self.buffer);
                    self.state = PasteBurstState::Watching;
                    self.consecutive_count = 1;
                    self.last_char_time = Some(now);
                    self.passthrough_chars.clear();
                    self.passthrough_chars.push(c);
                    CharAction::Flush(pasted)
                }
            }
        }
    }

    /// Check whether the idle timeout has elapsed and flush any buffered content.
    pub fn flush_if_due(&mut self, now: Instant) -> Option<String> {
        match &self.state {
            PasteBurstState::Buffering => {
                let elapsed = now.duration_since(self.last_char_time.unwrap_or(now));
                if elapsed.as_millis() >= IDLE_TIMEOUT_MS {
                    let pasted = std::mem::take(&mut self.buffer);
                    self.reset();
                    Some(pasted)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Flush buffered content immediately (for non-char input during burst).
    pub fn flush_now(&mut self) -> Option<String> {
        match &self.state {
            PasteBurstState::Buffering => {
                let pasted = std::mem::take(&mut self.buffer);
                self.reset();
                Some(pasted)
            }
            _ => None,
        }
    }

    /// Whether we are currently accumulating a paste burst.
    pub fn is_buffering(&self) -> bool {
        self.state == PasteBurstState::Buffering
    }

    /// Reset all state.
    pub fn clear_after_explicit_paste(&mut self) {
        self.reset();
    }

    fn reset(&mut self) {
        self.state = PasteBurstState::Idle;
        self.consecutive_count = 0;
        self.last_char_time = None;
        self.passthrough_chars.clear();
        self.buffer.clear();
    }
}

impl Default for PasteBurst {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_first_char_passthrough() {
        let mut pb = PasteBurst::new();
        let now = Instant::now();
        assert!(matches!(pb.push_char('a', now), CharAction::Passthrough));
    }

    #[test]
    fn test_second_char_passthrough() {
        let mut pb = PasteBurst::new();
        let now = Instant::now();
        pb.push_char('a', now);
        assert!(matches!(
            pb.push_char('b', now + Duration::from_millis(5)),
            CharAction::Passthrough
        ));
    }

    #[test]
    fn test_third_char_triggers_retro_flush() {
        let mut pb = PasteBurst::new();
        let now = Instant::now();
        pb.push_char('a', now);
        pb.push_char('b', now + Duration::from_millis(5));

        match pb.push_char('c', now + Duration::from_millis(10)) {
            CharAction::RetroFlush { text, retro_count } => {
                assert_eq!(text, "abc");
                assert_eq!(retro_count, 2);
            }
            other => panic!("expected RetroFlush, got {:?}", other),
        }
        assert!(pb.is_buffering());
    }

    #[test]
    fn test_buffering_after_retro_flush() {
        let mut pb = PasteBurst::new();
        let now = Instant::now();
        pb.push_char('a', now);
        pb.push_char('b', now + Duration::from_millis(5));
        pb.push_char('c', now + Duration::from_millis(10));
        assert!(pb.is_buffering());

        assert!(matches!(
            pb.push_char('d', now + Duration::from_millis(15)),
            CharAction::Buffering
        ));
        assert!(matches!(
            pb.push_char('e', now + Duration::from_millis(20)),
            CharAction::Buffering
        ));
    }

    #[test]
    fn test_flush_after_idle_timeout() {
        let mut pb = PasteBurst::new();
        let now = Instant::now();
        pb.push_char('a', now);
        pb.push_char('b', now + Duration::from_millis(5));
        pb.push_char('c', now + Duration::from_millis(10));
        pb.push_char('d', now + Duration::from_millis(15));
        assert!(pb.is_buffering());

        let result = pb.flush_if_due(now + Duration::from_millis(200));
        assert_eq!(result, Some("d".to_string()));
        assert!(!pb.is_buffering());
    }

    #[test]
    fn test_slow_typing_not_burst() {
        let mut pb = PasteBurst::new();
        let now = Instant::now();
        assert!(matches!(pb.push_char('a', now), CharAction::Passthrough));

        // 100ms later → not a burst
        assert!(matches!(
            pb.push_char('b', now + Duration::from_millis(100)),
            CharAction::Passthrough
        ));
        assert!(!pb.is_buffering());

        assert!(matches!(
            pb.push_char('c', now + Duration::from_millis(200)),
            CharAction::Passthrough
        ));
        assert!(!pb.is_buffering());
    }

    #[test]
    fn test_flush_now_during_burst() {
        let mut pb = PasteBurst::new();
        let now = Instant::now();
        pb.push_char('a', now);
        pb.push_char('b', now + Duration::from_millis(5));
        pb.push_char('c', now + Duration::from_millis(10));
        pb.push_char('d', now + Duration::from_millis(15));
        assert!(pb.is_buffering());

        let result = pb.flush_now();
        assert_eq!(result, Some("d".to_string()));
        assert!(!pb.is_buffering());
    }

    #[test]
    fn test_clear_after_explicit_paste() {
        let mut pb = PasteBurst::new();
        let now = Instant::now();
        pb.push_char('a', now);
        pb.push_char('b', now + Duration::from_millis(5));
        pb.push_char('c', now + Duration::from_millis(10));
        assert!(pb.is_buffering());
        pb.clear_after_explicit_paste();
        assert!(!pb.is_buffering());
    }

    #[test]
    fn test_gap_flushes_during_buffering() {
        let mut pb = PasteBurst::new();
        let now = Instant::now();
        pb.push_char('a', now);
        pb.push_char('b', now + Duration::from_millis(5));
        pb.push_char('c', now + Duration::from_millis(10));
        pb.push_char('d', now + Duration::from_millis(15));
        assert!(pb.is_buffering());

        // Gap of 50ms → flush "d", start new watch for 'e'
        match pb.push_char('e', now + Duration::from_millis(70)) {
            CharAction::Flush(text) => assert_eq!(text, "d"),
            other => panic!("expected Flush, got {:?}", other),
        }
        assert!(!pb.is_buffering());
    }

    #[test]
    fn test_exactly_min_burst_chars() {
        let mut pb = PasteBurst::new();
        let now = Instant::now();
        pb.push_char('a', now);
        pb.push_char('b', now + Duration::from_millis(5));

        // 3rd char confirms burst
        match pb.push_char('c', now + Duration::from_millis(10)) {
            CharAction::RetroFlush { text, retro_count } => {
                assert_eq!(text, "abc");
                assert_eq!(retro_count, 2);
            }
            other => panic!("expected RetroFlush, got {:?}", other),
        }
    }

    #[test]
    fn test_passthrough_chars_are_captured_correctly() {
        let mut pb = PasteBurst::new();
        let now = Instant::now();
        pb.push_char('a', now);
        pb.push_char('b', now + Duration::from_millis(5));
        pb.push_char('c', now + Duration::from_millis(10));
        // passthrough_chars should now be consumed by RetroFlush

        // Additional chars go to buffer
        pb.push_char('d', now + Duration::from_millis(15));
        pb.push_char('e', now + Duration::from_millis(20));

        let result = pb.flush_if_due(now + Duration::from_millis(200));
        assert_eq!(result, Some("de".to_string()));
    }
}
