//! Keyboard input handling for terminal applications
//!
//! In Rust + crossterm, raw escape sequences are already parsed by crossterm
//! into structured KeyEvent events. This module maps crossterm events to our
//! string-based key identifiers (KeyId).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Cached Kitty keyboard protocol state
static mut KITTY_PROTOCOL_ACTIVE: bool = false;

/// Set the Kitty keyboard protocol state
pub fn set_kitty_protocol_active(active: bool) {
    unsafe {
        KITTY_PROTOCOL_ACTIVE = active;
    }
}

/// Query whether Kitty keyboard protocol is currently active
pub fn is_kitty_protocol_active() -> bool {
    unsafe { KITTY_PROTOCOL_ACTIVE }
}

/// Type alias for key identifiers (e.g., "ctrl+c", "escape", "shift+enter")
pub type KeyId = String;

/// Helper for constructing typed key identifiers
pub struct Key;

#[allow(non_upper_case_globals)]
impl Key {
    // Special keys
    pub const Escape: &'static str = "escape";
    pub const Esc: &'static str = "esc";
    pub const Enter: &'static str = "enter";
    pub const Return: &'static str = "return";
    pub const Tab: &'static str = "tab";
    pub const Space: &'static str = "space";
    pub const Backspace: &'static str = "backspace";
    pub const Delete: &'static str = "delete";
    pub const Insert: &'static str = "insert";
    pub const Home: &'static str = "home";
    pub const End: &'static str = "end";
    pub const PageUp: &'static str = "pageUp";
    pub const PageDown: &'static str = "pageDown";
    pub const Up: &'static str = "up";
    pub const Down: &'static str = "down";
    pub const Left: &'static str = "left";
    pub const Right: &'static str = "right";

    // F-keys
    pub const F1: &'static str = "f1";
    pub const F2: &'static str = "f2";
    pub const F3: &'static str = "f3";
    pub const F4: &'static str = "f4";
    pub const F5: &'static str = "f5";
    pub const F6: &'static str = "f6";
    pub const F7: &'static str = "f7";
    pub const F8: &'static str = "f8";
    pub const F9: &'static str = "f9";
    pub const F10: &'static str = "f10";
    pub const F11: &'static str = "f11";
    pub const F12: &'static str = "f12";

    /// Create a modifier key identifier
    pub fn ctrl(key: &str) -> String {
        format!("ctrl+{}", key)
    }

    pub fn shift(key: &str) -> String {
        format!("shift+{}", key)
    }

    pub fn alt(key: &str) -> String {
        format!("alt+{}", key)
    }

    pub fn super_key(key: &str) -> String {
        format!("super+{}", key)
    }

    pub fn ctrl_shift(key: &str) -> String {
        format!("ctrl+shift+{}", key)
    }

    pub fn ctrl_alt(key: &str) -> String {
        format!("ctrl+alt+{}", key)
    }

    pub fn shift_alt(key: &str) -> String {
        format!("shift+alt+{}", key)
    }

    pub fn ctrl_super(key: &str) -> String {
        format!("ctrl+super+{}", key)
    }
}

/// Check if a key event matches a key identifier string
pub fn matches_key(event: &KeyEvent, key_id: &str) -> bool {
    let parsed = parse_key_id(key_id);
    let (expected_key, expected_mods) = match parsed {
        Some(k) => k,
        None => return false,
    };

    let actual_mods = event.modifiers;

    // Check modifier match (only check if any modifiers are specified)
    let mods_match = if expected_mods.is_empty() {
        actual_mods.is_empty() || actual_mods == KeyModifiers::NONE
    } else {
        let shift = expected_mods.contains(KeyModifiers::SHIFT);
        let ctrl = expected_mods.contains(KeyModifiers::CONTROL);
        let alt = expected_mods.contains(KeyModifiers::ALT);
        let super_mod = expected_mods.contains(KeyModifiers::SUPER);

        (shift == actual_mods.contains(KeyModifiers::SHIFT) || !shift)
            && (ctrl == actual_mods.contains(KeyModifiers::CONTROL) || !ctrl)
            && (alt == actual_mods.contains(KeyModifiers::ALT) || !alt)
            && (super_mod == actual_mods.contains(KeyModifiers::SUPER) || !super_mod)
    };

    if !mods_match {
        return false;
    }

    key_code_matches(&event.code, expected_key)
}

fn key_code_matches(actual: &KeyCode, expected: &str) -> bool {
    match actual {
        KeyCode::Esc => matches!(expected, "escape" | "esc"),
        KeyCode::Enter => matches!(expected, "enter" | "return"),
        KeyCode::Tab => expected == "tab",
        KeyCode::Backspace => expected == "backspace",
        KeyCode::Delete => expected == "delete",
        KeyCode::Insert => expected == "insert",
        KeyCode::Home => expected == "home",
        KeyCode::End => expected == "end",
        KeyCode::PageUp => expected == "pageUp",
        KeyCode::PageDown => expected == "pageDown",
        KeyCode::Up => expected == "up",
        KeyCode::Down => expected == "down",
        KeyCode::Left => expected == "left",
        KeyCode::Right => expected == "right",
        KeyCode::F(n) => expected == format!("f{}", n),
        KeyCode::Char(c) => {
            if c == &' ' {
                expected == "space"
            } else {
                expected.len() == 1 && expected.starts_with(*c)
            }
        }
        _ => false,
    }
}

/// Parse a key event and return the key identifier string
pub fn parse_key(event: &KeyEvent) -> Option<String> {
    let key_name = key_code_name(&event.code)?;

    let mut parts = Vec::new();
    if event.modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("ctrl");
    }
    if event.modifiers.contains(KeyModifiers::ALT) {
        parts.push("alt");
    }
    if event.modifiers.contains(KeyModifiers::SHIFT) {
        // Don't add "shift" for uppercase letters (they're already uppercase)
        if !matches!(event.code, KeyCode::Char(c) if c.is_uppercase()) {
            parts.push("shift");
        }
    }
    if event.modifiers.contains(KeyModifiers::SUPER) {
        parts.push("super");
    }

    if parts.is_empty() {
        Some(key_name.to_string())
    } else {
        parts.push(key_name);
        Some(parts.join("+"))
    }
}

fn key_code_name(code: &KeyCode) -> Option<&'static str> {
    match code {
        KeyCode::Esc => Some("escape"),
        KeyCode::Enter => Some("enter"),
        KeyCode::Tab => Some("tab"),
        KeyCode::Backspace => Some("backspace"),
        KeyCode::Delete => Some("delete"),
        KeyCode::Insert => Some("insert"),
        KeyCode::Home => Some("home"),
        KeyCode::End => Some("end"),
        KeyCode::PageUp => Some("pageUp"),
        KeyCode::PageDown => Some("pageDown"),
        KeyCode::Up => Some("up"),
        KeyCode::Down => Some("down"),
        KeyCode::Left => Some("left"),
        KeyCode::Right => Some("right"),
        KeyCode::Char(' ') => Some("space"),
        KeyCode::F(n) => {
            let name = match n {
                1 => "f1",
                2 => "f2",
                3 => "f3",
                4 => "f4",
                5 => "f5",
                6 => "f6",
                7 => "f7",
                8 => "f8",
                9 => "f9",
                10 => "f10",
                11 => "f11",
                12 => "f12",
                _ => return None,
            };
            Some(name)
        }
        KeyCode::Char(c) => {
            // For single chars, return the char itself as a static str
            // We use a leak approach here since the set of possible chars is bounded
            let s: &'static str = Box::leak(c.to_string().into_boxed_str());
            Some(s)
        }
        _ => None,
    }
}
fn parse_key_id(key_id: &str) -> Option<(&str, KeyModifiers)> {
    let parts: Vec<&str> = key_id.split('+').collect();
    if parts.is_empty() {
        return None;
    }

    let key = parts.last().copied()?;
    let mut mods = KeyModifiers::NONE;

    for m in &parts[..parts.len() - 1] {
        match *m {
            "ctrl" => mods.insert(KeyModifiers::CONTROL),
            "shift" => mods.insert(KeyModifiers::SHIFT),
            "alt" | "option" => mods.insert(KeyModifiers::ALT),
            "super" | "meta" | "cmd" | "command" | "win" | "windows" => {
                mods.insert(KeyModifiers::SUPER);
            }
            _ => {}
        }
    }

    Some((key, mods))
}

/// Decode a printable character from a key event
pub fn decode_printable_key(event: &KeyEvent) -> Option<char> {
    match event.code {
        KeyCode::Char(c) => {
            // Only accept unmodified or Shift-modified chars
            if event.modifiers.contains(KeyModifiers::CONTROL)
                || event.modifiers.contains(KeyModifiers::ALT)
                || event.modifiers.contains(KeyModifiers::SUPER)
            {
                return None;
            }
            if c.is_ascii_control() {
                return None;
            }
            Some(c)
        }
        KeyCode::Enter => None,
        KeyCode::Tab => None,
        KeyCode::Backspace => None,
        KeyCode::Esc => None,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn test_simple_key() {
        let event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(matches_key(&event, "a"));
    }

    #[test]
    fn test_ctrl_key() {
        let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(matches_key(&event, "ctrl+c"));
    }

    #[test]
    fn test_escape() {
        let event = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert!(matches_key(&event, "escape"));
        assert!(matches_key(&event, "esc"));
    }

    #[test]
    fn test_arrow() {
        let event = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        assert!(matches_key(&event, "up"));
    }

    #[test]
    fn test_parse_key() {
        let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let parsed = parse_key(&event);
        assert_eq!(parsed.as_deref(), Some("ctrl+c"));
    }

    #[test]
    fn test_decode_printable() {
        let event = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        assert_eq!(decode_printable_key(&event), Some('x'));

        let ctrl_event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(decode_printable_key(&ctrl_event), None);
    }
}
