//! Keyboard enhancement protocol support.
//!
//! Uses the Kitty keyboard protocol (`\x1b[>1u`) to get proper modifier
//! reporting for modified keys like Ctrl+Enter, Shift+Enter.
//! When active, crossterm reports `KeyCode::Enter` + `KeyModifiers::CONTROL`
//! for Ctrl+Enter, eliminating the need for `GetAsyncKeyState` fallbacks.
//!
//! Aligns with the approach used by Codex (codex-rs).

use std::sync::atomic::AtomicBool;

/// Whether keyboard enhancement has been successfully enabled.
static ENHANCED_KEYS_ENABLED: AtomicBool = AtomicBool::new(false);

/// Check whether keyboard enhancement is active.
pub fn is_enhanced() -> bool {
    ENHANCED_KEYS_ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Probe whether the terminal supports keyboard enhancement (Kitty protocol).
///
/// On Windows: uses `crossterm::terminal::supports_keyboard_enhancement()`
/// which queries the console mode and sends a probe sequence.
/// On Unix: sends a CSI u query with a 100ms timeout and parses the response.
///
/// This is the KEY difference from just calling `crossterm::execute!` with
/// `PushKeyboardEnhancementFlags`, which succeeds even on non-supporting
/// terminals because writing the escape sequence to stdout never fails.
/// Only terminals that genuinely respond to the probe are trusted.
fn probe() -> bool {
    crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false)
}

/// Enable the keyboard enhancement protocol.
///
/// First probes the terminal to verify it actually supports the protocol
/// (using `crossterm::terminal::supports_keyboard_enhancement()`), then
/// sends `DISAMBIGUATE_ESCAPE_CODES | REPORT_EVENT_TYPES | REPORT_ALTERNATE_KEYS`.
///
/// On terminals that support the Kitty protocol (Windows Terminal,
/// Alacritty, Wezterm, iTerm2, kitty, etc.), this makes modified Enter keys
/// report the correct modifiers. Unsupported terminals are correctly detected
/// and `is_enhanced()` returns `false` so callers can fall back to safe defaults.
pub fn enable() {
    use crossterm::event::{KeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
    use std::io::Write;

    if !probe() {
        return;
    }

    let result = crossterm::execute!(
        std::io::stdout(),
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS,
        ),
    );
    if result.is_ok() {
        let _ = std::io::stdout().flush();
        ENHANCED_KEYS_ENABLED.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Disable the keyboard enhancement protocol.
///
/// Pops the previously pushed enhancement flags, restoring the terminal
/// to its default key reporting behavior.
pub fn disable() {
    use crossterm::event::PopKeyboardEnhancementFlags;
    use std::io::Write;

    let _ = crossterm::execute!(std::io::stdout(), PopKeyboardEnhancementFlags);
    let _ = std::io::stdout().flush();
    let _ = ENHANCED_KEYS_ENABLED.swap(false, std::sync::atomic::Ordering::Relaxed);
}
