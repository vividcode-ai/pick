use std::collections::HashMap;

use super::{AppKeybindings, Keybinding, KeybindingDefinitions, KeybindingValue};

pub fn default_keybindings() -> KeybindingDefinitions {
    let mut k = HashMap::new();
    for (key, binding) in editor_keybindings() {
        k.insert(key, binding);
    }
    for (key, binding) in navigation_keybindings() {
        k.insert(key, binding);
    }
    for (key, binding) in mode_keybindings() {
        k.insert(key, binding);
    }
    k
}

fn editor_keybindings() -> Vec<(String, Keybinding)> {
    vec![
        (
            "tui.editor.cursorUp".into(),
            keybinding("ctrl+p", "Cursor up"),
        ),
        (
            "tui.editor.cursorDown".into(),
            keybinding("ctrl+n", "Cursor down"),
        ),
        (
            "tui.editor.cursorLeft".into(),
            keybinding("ctrl+b", "Cursor left"),
        ),
        (
            "tui.editor.cursorRight".into(),
            keybinding("ctrl+f", "Cursor right"),
        ),
        (
            "tui.editor.cursorWordLeft".into(),
            keybinding("alt+b", "Cursor word left"),
        ),
        (
            "tui.editor.cursorWordRight".into(),
            keybinding("alt+f", "Cursor word right"),
        ),
        (
            "tui.editor.cursorLineStart".into(),
            keybinding("ctrl+a", "Cursor line start"),
        ),
        (
            "tui.editor.cursorLineEnd".into(),
            keybinding("ctrl+e", "Cursor line end"),
        ),
        (
            "tui.editor.jumpForward".into(),
            keybinding("ctrl+right", "Jump forward"),
        ),
        (
            "tui.editor.jumpBackward".into(),
            keybinding("ctrl+left", "Jump backward"),
        ),
        ("tui.editor.pageUp".into(), keybinding("alt+up", "Page up")),
        (
            "tui.editor.pageDown".into(),
            keybinding("alt+down", "Page down"),
        ),
        (
            "tui.editor.deleteCharBackward".into(),
            keybinding("backspace", "Delete char backward"),
        ),
        (
            "tui.editor.deleteCharForward".into(),
            keybinding("delete", "Delete char forward"),
        ),
        (
            "tui.editor.deleteWordBackward".into(),
            keybinding("alt+backspace", "Delete word backward"),
        ),
        (
            "tui.editor.deleteWordForward".into(),
            keybinding("alt+delete", "Delete word forward"),
        ),
        (
            "tui.editor.deleteToLineStart".into(),
            keybinding("ctrl+u", "Delete to line start"),
        ),
        (
            "tui.editor.deleteToLineEnd".into(),
            keybinding("ctrl+k", "Delete to line end"),
        ),
        ("tui.editor.yank".into(), keybinding("ctrl+y", "Yank")),
        ("tui.editor.yankPop".into(), keybinding("alt+y", "Yank pop")),
        ("tui.editor.undo".into(), keybinding("ctrl+z", "Undo")),
        ("tui.input.newLine".into(), keybinding("enter", "New line")),
        (
            "tui.input.submit".into(),
            keybinding("ctrl+enter", "Submit"),
        ),
        ("tui.input.tab".into(), keybinding("tab", "Tab")),
        ("tui.input.copy".into(), keybinding("ctrl+shift+c", "Copy")),
    ]
}

fn navigation_keybindings() -> Vec<(String, Keybinding)> {
    vec![
        ("tui.select.up".into(), keybinding("up", "Select up")),
        ("tui.select.down".into(), keybinding("down", "Select down")),
        (
            "tui.select.pageUp".into(),
            keybinding("pageup", "Select page up"),
        ),
        (
            "tui.select.pageDown".into(),
            keybinding("pagedown", "Select page down"),
        ),
        (
            "tui.select.confirm".into(),
            keybinding("enter", "Select confirm"),
        ),
        (
            "tui.select.cancel".into(),
            keybinding("escape", "Select cancel"),
        ),
    ]
}

fn mode_keybindings() -> Vec<(String, Keybinding)> {
    vec![
        (
            AppKeybindings::INTERRUPT.into(),
            Keybinding {
                default_keys: single("escape"),
                description: "Cancel or abort".into(),
            },
        ),
        (
            AppKeybindings::CLEAR.into(),
            Keybinding {
                default_keys: single("ctrl+c"),
                description: "Clear editor".into(),
            },
        ),
        (
            AppKeybindings::EXIT.into(),
            Keybinding {
                default_keys: single("ctrl+d"),
                description: "Exit when editor is empty".into(),
            },
        ),
        (
            AppKeybindings::SUSPEND.into(),
            Keybinding {
                default_keys: multiple(vec!["ctrl+z".into()]),
                description: "Suspend to background".into(),
            },
        ),
        (
            AppKeybindings::THINKING_CYCLE.into(),
            Keybinding {
                default_keys: single("shift+tab"),
                description: "Cycle thinking level".into(),
            },
        ),
        (
            AppKeybindings::MODEL_CYCLE_FORWARD.into(),
            Keybinding {
                default_keys: single("ctrl+p"),
                description: "Cycle to next model".into(),
            },
        ),
        (
            AppKeybindings::MODEL_CYCLE_BACKWARD.into(),
            Keybinding {
                default_keys: single("shift+ctrl+p"),
                description: "Cycle to previous model".into(),
            },
        ),
        (
            AppKeybindings::MODEL_SELECT.into(),
            Keybinding {
                default_keys: single("ctrl+l"),
                description: "Open model selector".into(),
            },
        ),
        (
            AppKeybindings::TOOLS_EXPAND.into(),
            Keybinding {
                default_keys: single("ctrl+o"),
                description: "Toggle tool output".into(),
            },
        ),
        (
            AppKeybindings::THINKING_TOGGLE.into(),
            Keybinding {
                default_keys: single("ctrl+t"),
                description: "Toggle thinking blocks".into(),
            },
        ),
        (
            AppKeybindings::SESSION_TOGGLE_NAMED_FILTER.into(),
            Keybinding {
                default_keys: single("ctrl+n"),
                description: "Toggle named session filter".into(),
            },
        ),
        (
            AppKeybindings::EDITOR_EXTERNAL.into(),
            Keybinding {
                default_keys: single("ctrl+g"),
                description: "Open external editor".into(),
            },
        ),
        (
            AppKeybindings::MESSAGE_FOLLOW_UP.into(),
            Keybinding {
                default_keys: single("alt+enter"),
                description: "Queue follow-up message".into(),
            },
        ),
        (
            AppKeybindings::MESSAGE_DEQUEUE.into(),
            Keybinding {
                default_keys: single("alt+up"),
                description: "Restore queued messages".into(),
            },
        ),
        (
            AppKeybindings::CLIPBOARD_PASTE_IMAGE.into(),
            Keybinding {
                default_keys: single("alt+v"),
                description: "Paste image from clipboard".into(),
            },
        ),
        (
            AppKeybindings::SESSION_NEW.into(),
            Keybinding {
                default_keys: multiple(vec![]),
                description: "Start a new session".into(),
            },
        ),
        (
            AppKeybindings::SESSION_TREE.into(),
            Keybinding {
                default_keys: multiple(vec![]),
                description: "Open session tree".into(),
            },
        ),
        (
            AppKeybindings::SESSION_FORK.into(),
            Keybinding {
                default_keys: multiple(vec![]),
                description: "Fork current session".into(),
            },
        ),
        (
            AppKeybindings::SESSION_RESUME.into(),
            Keybinding {
                default_keys: multiple(vec![]),
                description: "Resume a session".into(),
            },
        ),
        (
            AppKeybindings::TREE_FOLD_OR_UP.into(),
            Keybinding {
                default_keys: multiple(vec!["ctrl+left".into(), "alt+left".into()]),
                description: "Fold tree branch or move up".into(),
            },
        ),
        (
            AppKeybindings::TREE_UNFOLD_OR_DOWN.into(),
            Keybinding {
                default_keys: multiple(vec!["ctrl+right".into(), "alt+right".into()]),
                description: "Unfold tree branch or move down".into(),
            },
        ),
        (
            AppKeybindings::TREE_EDIT_LABEL.into(),
            Keybinding {
                default_keys: single("shift+l"),
                description: "Edit tree label".into(),
            },
        ),
        (
            AppKeybindings::TREE_TOGGLE_LABEL_TIMESTAMP.into(),
            Keybinding {
                default_keys: single("shift+t"),
                description: "Toggle tree label timestamps".into(),
            },
        ),
        (
            AppKeybindings::SESSION_TOGGLE_PATH.into(),
            Keybinding {
                default_keys: single("ctrl+p"),
                description: "Toggle session path display".into(),
            },
        ),
        (
            AppKeybindings::SESSION_TOGGLE_SORT.into(),
            Keybinding {
                default_keys: single("ctrl+s"),
                description: "Toggle session sort mode".into(),
            },
        ),
        (
            AppKeybindings::SESSION_RENAME.into(),
            Keybinding {
                default_keys: single("ctrl+r"),
                description: "Rename session".into(),
            },
        ),
        (
            AppKeybindings::SESSION_DELETE.into(),
            Keybinding {
                default_keys: single("ctrl+d"),
                description: "Delete session".into(),
            },
        ),
        (
            AppKeybindings::SESSION_DELETE_NONINVASIVE.into(),
            Keybinding {
                default_keys: single("ctrl+backspace"),
                description: "Delete session when query is empty".into(),
            },
        ),
    ]
}

fn keybinding(keys: &str, description: &str) -> Keybinding {
    Keybinding {
        default_keys: single(keys),
        description: description.into(),
    }
}

fn single(keys: &str) -> KeybindingValue {
    KeybindingValue::Single(keys.into())
}

fn multiple(keys: Vec<String>) -> KeybindingValue {
    KeybindingValue::Multiple(keys)
}
