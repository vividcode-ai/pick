//! Keybinding definitions and management.

use std::collections::HashMap;

use super::keys::matches_key;

/// A keybinding identifier type
pub type Keybinding = &'static str;

/// Available keybinding definitions
#[derive(Debug, Clone)]
pub struct KeybindingDefinition {
    pub default_keys: Vec<String>,
    pub description: &'static str,
}

/// A resolved key (single key or list of alternatives)
#[derive(Debug, Clone)]
pub enum KeyOrList {
    Single(String),
    Multiple(Vec<String>),
}

/// A keybinding conflict: same key mapped to multiple actions
#[derive(Debug, Clone)]
pub struct KeybindingConflict {
    pub key: String,
    pub keybindings: Vec<String>,
}

/// User-facing keybinding config (maps action names to key IDs)
pub type KeybindingsConfig = HashMap<String, KeyOrList>;

/// Manager for keybindings with user override and conflict detection support
pub struct KeybindingsManager {
    definitions: HashMap<String, KeybindingDefinition>,
    user_bindings: KeybindingsConfig,
    keys_by_id: HashMap<String, Vec<String>>,
    conflicts: Vec<KeybindingConflict>,
}

impl KeybindingsManager {
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
            user_bindings: HashMap::new(),
            keys_by_id: HashMap::new(),
            conflicts: Vec::new(),
        }
    }

    /// Create from definitions and optional user overrides
    pub fn from_definitions(
        definitions: HashMap<String, KeybindingDefinition>,
        user_bindings: KeybindingsConfig,
    ) -> Self {
        let mut mgr = Self {
            definitions,
            user_bindings,
            keys_by_id: HashMap::new(),
            conflicts: Vec::new(),
        };
        mgr.rebuild();
        mgr
    }

    /// Rebuild internal key cache and detect conflicts
    fn rebuild(&mut self) {
        self.keys_by_id.clear();
        self.conflicts.clear();

        // Collect user claims (which keys are assigned to which actions)
        let mut user_claims: HashMap<String, Vec<String>> = HashMap::new();
        for (action, keys) in &self.user_bindings {
            if !self.definitions.contains_key(action) {
                continue;
            }
            let key_list = match keys {
                KeyOrList::Single(k) => vec![k.clone()],
                KeyOrList::Multiple(ks) => ks.clone(),
            };
            for key in key_list {
                user_claims.entry(key).or_default().push(action.clone());
            }
        }

        // Detect conflicts among user claims
        for (key, actions) in &user_claims {
            if actions.len() > 1 {
                self.conflicts.push(KeybindingConflict {
                    key: key.clone(),
                    keybindings: actions.clone(),
                });
            }
        }

        // Resolve keys for each definition (user override > default)
        for (id, definition) in &self.definitions {
            let keys = match self.user_bindings.get(id) {
                Some(KeyOrList::Single(k)) => vec![k.clone()],
                Some(KeyOrList::Multiple(ks)) => ks.clone(),
                None => definition.default_keys.clone(),
            };
            self.keys_by_id.insert(id.clone(), keys);
        }
    }

    /// Check if an input event matches a keybinding
    pub fn matches(&self, event: &crossterm::event::KeyEvent, keybinding: &str) -> bool {
        let keys = match self.keys_by_id.get(keybinding) {
            Some(k) => k,
            None => return false,
        };
        for key in keys {
            if matches_key(event, key) {
                return true;
            }
        }
        false
    }

    /// Get the resolved keys for a keybinding
    pub fn get_keys(&self, keybinding: &str) -> Vec<String> {
        self.keys_by_id
            .get(keybinding)
            .cloned()
            .unwrap_or_default()
    }

    /// Get the definition for a keybinding
    pub fn get_definition(&self, keybinding: &str) -> Option<&KeybindingDefinition> {
        self.definitions.get(keybinding)
    }

    /// Get all detected keybinding conflicts
    pub fn get_conflicts(&self) -> &[KeybindingConflict] {
        &self.conflicts
    }

    /// Set user keybinding overrides and rebuild
    pub fn set_user_bindings(&mut self, user_bindings: KeybindingsConfig) {
        self.user_bindings = user_bindings;
        self.rebuild();
    }

    /// Get current user overrides
    pub fn get_user_bindings(&self) -> &KeybindingsConfig {
        &self.user_bindings
    }

    /// Get merged default + user bindings
    pub fn get_resolved_bindings(&self) -> KeybindingsConfig {
        let mut resolved = KeybindingsConfig::new();
        for (id, keys) in &self.keys_by_id {
            if keys.len() == 1 {
                resolved.insert(id.clone(), KeyOrList::Single(keys[0].clone()));
            } else {
                resolved.insert(id.clone(), KeyOrList::Multiple(keys.clone()));
            }
        }
        resolved
    }

    /// Register a new keybinding definition
    pub fn register(&mut self, id: &str, keys: Vec<String>, description: &'static str) {
        self.definitions.insert(
            id.to_string(),
            KeybindingDefinition {
                default_keys: keys,
                description,
            },
        );
        self.rebuild();
    }

    /// Get all registered definitions
    pub fn get_all_definitions(&self) -> &HashMap<String, KeybindingDefinition> {
        &self.definitions
    }

    /// Find the keybinding action that matches an event
    pub fn find_action(&self, event: &crossterm::event::KeyEvent) -> Option<String> {
        for (id, keys) in &self.keys_by_id {
            for key in keys {
                if matches_key(event, key) {
                    return Some(id.clone());
                }
            }
        }
        None
    }
}

impl Default for KeybindingsManager {
    fn default() -> Self {
        Self::from_definitions(default_tui_keybindings(), HashMap::new())
    }
}

/// Build the default TUI keybinding definitions
fn default_tui_keybindings() -> HashMap<String, KeybindingDefinition> {
    let mut map = HashMap::new();

    // Editor navigation
    map.insert("tui.editor.cursorUp".to_string(), KeybindingDefinition {
        default_keys: vec!["up".to_string()],
        description: "Move cursor up",
    });
    map.insert("tui.editor.cursorDown".to_string(), KeybindingDefinition {
        default_keys: vec!["down".to_string()],
        description: "Move cursor down",
    });
    map.insert("tui.editor.cursorLeft".to_string(), KeybindingDefinition {
        default_keys: vec!["left".to_string(), "ctrl+b".to_string()],
        description: "Move cursor left",
    });
    map.insert("tui.editor.cursorRight".to_string(), KeybindingDefinition {
        default_keys: vec!["right".to_string(), "ctrl+f".to_string()],
        description: "Move cursor right",
    });
    map.insert("tui.editor.cursorWordLeft".to_string(), KeybindingDefinition {
        default_keys: vec!["alt+left".to_string(), "ctrl+left".to_string(), "alt+b".to_string()],
        description: "Move cursor word left",
    });
    map.insert("tui.editor.cursorWordRight".to_string(), KeybindingDefinition {
        default_keys: vec!["alt+right".to_string(), "ctrl+right".to_string(), "alt+f".to_string()],
        description: "Move cursor word right",
    });
    map.insert("tui.editor.cursorLineStart".to_string(), KeybindingDefinition {
        default_keys: vec!["home".to_string(), "ctrl+a".to_string()],
        description: "Move to line start",
    });
    map.insert("tui.editor.cursorLineEnd".to_string(), KeybindingDefinition {
        default_keys: vec!["end".to_string(), "ctrl+e".to_string()],
        description: "Move to line end",
    });
    map.insert("tui.editor.jumpForward".to_string(), KeybindingDefinition {
        default_keys: vec!["ctrl+]".to_string()],
        description: "Jump forward to character",
    });
    map.insert("tui.editor.jumpBackward".to_string(), KeybindingDefinition {
        default_keys: vec!["ctrl+alt+]".to_string()],
        description: "Jump backward to character",
    });
    map.insert("tui.editor.pageUp".to_string(), KeybindingDefinition {
        default_keys: vec!["pageUp".to_string()],
        description: "Page up",
    });
    map.insert("tui.editor.pageDown".to_string(), KeybindingDefinition {
        default_keys: vec!["pageDown".to_string()],
        description: "Page down",
    });

    // Editor editing
    map.insert("tui.editor.deleteCharBackward".to_string(), KeybindingDefinition {
        default_keys: vec!["backspace".to_string()],
        description: "Delete character backward",
    });
    map.insert("tui.editor.deleteCharForward".to_string(), KeybindingDefinition {
        default_keys: vec!["delete".to_string(), "ctrl+d".to_string()],
        description: "Delete character forward",
    });
    map.insert("tui.editor.deleteWordBackward".to_string(), KeybindingDefinition {
        default_keys: vec!["ctrl+w".to_string(), "alt+backspace".to_string()],
        description: "Delete word backward",
    });
    map.insert("tui.editor.deleteWordForward".to_string(), KeybindingDefinition {
        default_keys: vec!["alt+d".to_string(), "alt+delete".to_string()],
        description: "Delete word forward",
    });
    map.insert("tui.editor.deleteToLineStart".to_string(), KeybindingDefinition {
        default_keys: vec!["ctrl+u".to_string()],
        description: "Delete to line start",
    });
    map.insert("tui.editor.deleteToLineEnd".to_string(), KeybindingDefinition {
        default_keys: vec!["ctrl+k".to_string()],
        description: "Delete to line end",
    });
    map.insert("tui.editor.yank".to_string(), KeybindingDefinition {
        default_keys: vec!["ctrl+y".to_string()],
        description: "Yank",
    });
    map.insert("tui.editor.yankPop".to_string(), KeybindingDefinition {
        default_keys: vec!["alt+y".to_string()],
        description: "Yank pop",
    });
    map.insert("tui.editor.undo".to_string(), KeybindingDefinition {
        default_keys: vec!["ctrl+-".to_string()],
        description: "Undo",
    });

    // Input actions
    map.insert("tui.input.newLine".to_string(), KeybindingDefinition {
        default_keys: vec!["shift+enter".to_string()],
        description: "Insert newline",
    });
    map.insert("tui.input.submit".to_string(), KeybindingDefinition {
        default_keys: vec!["enter".to_string()],
        description: "Submit input",
    });
    map.insert("tui.input.tab".to_string(), KeybindingDefinition {
        default_keys: vec!["tab".to_string()],
        description: "Tab / autocomplete",
    });
    map.insert("tui.input.copy".to_string(), KeybindingDefinition {
        default_keys: vec!["ctrl+c".to_string()],
        description: "Copy selection",
    });

    // Selection actions
    map.insert("tui.select.up".to_string(), KeybindingDefinition {
        default_keys: vec!["up".to_string()],
        description: "Move selection up",
    });
    map.insert("tui.select.down".to_string(), KeybindingDefinition {
        default_keys: vec!["down".to_string()],
        description: "Move selection down",
    });
    map.insert("tui.select.pageUp".to_string(), KeybindingDefinition {
        default_keys: vec!["pageUp".to_string()],
        description: "Selection page up",
    });
    map.insert("tui.select.pageDown".to_string(), KeybindingDefinition {
        default_keys: vec!["pageDown".to_string()],
        description: "Selection page down",
    });
    map.insert("tui.select.confirm".to_string(), KeybindingDefinition {
        default_keys: vec!["enter".to_string()],
        description: "Confirm selection",
    });
    map.insert("tui.select.cancel".to_string(), KeybindingDefinition {
        default_keys: vec!["escape".to_string(), "ctrl+c".to_string()],
        description: "Cancel selection",
    });

    map
}

// Global keybindings singleton
use std::sync::{Mutex, OnceLock};

fn global_keybindings_inner() -> &'static Mutex<Option<KeybindingsManager>> {
    static INSTANCE: OnceLock<Mutex<Option<KeybindingsManager>>> = OnceLock::new();
    INSTANCE.get_or_init(|| Mutex::new(None))
}

/// Set global keybindings
pub fn set_keybindings(mgr: KeybindingsManager) {
    if let Ok(mut guard) = global_keybindings_inner().lock() {
        *guard = Some(mgr);
    }
}

/// Get global keybindings (initialized with defaults on first call)
pub fn get_keybindings() -> std::sync::MutexGuard<'static, Option<KeybindingsManager>> {
    let mut guard = global_keybindings_inner().lock().unwrap();
    if guard.is_none() {
        *guard = Some(KeybindingsManager::default());
    }
    guard
}
