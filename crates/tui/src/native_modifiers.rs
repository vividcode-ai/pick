//! Native modifier key querying
//!
//! In Rust + crossterm, modifier state is available directly from KeyboardEnhancement
//! or through platform-specific APIs. This module provides a simplified interface.

/// Modifier key types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModifierKey {
    Shift,
    Control,
    Alt,
    Super,
}

impl ModifierKey {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "shift" => Some(Self::Shift),
            "control" | "ctrl" => Some(Self::Control),
            "alt" | "option" => Some(Self::Alt),
            "super" | "command" | "cmd" | "meta" | "win" | "windows" => Some(Self::Super),
            _ => None,
        }
    }
}
