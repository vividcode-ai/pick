//! Keybinding system with user binding loading and migration


pub mod defaults;

pub use defaults::default_keybindings;

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub type KeyId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeybindingValue {
    Single(String),
    Multiple(Vec<String>),
}

impl KeybindingValue {
    pub fn as_keys(&self) -> Vec<&str> {
        match self {
            KeybindingValue::Single(s) => vec![s.as_str()],
            KeybindingValue::Multiple(v) => v.iter().map(|s| s.as_str()).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keybinding {
    pub default_keys: KeybindingValue,
    pub description: String,
}

pub type KeybindingDefinitions = HashMap<String, Keybinding>;

pub type KeybindingsConfig = HashMap<String, KeybindingValue>;

pub struct AppKeybindings;

impl AppKeybindings {
    pub const INTERRUPT: &'static str = "app.interrupt";
    pub const CLEAR: &'static str = "app.clear";
    pub const EXIT: &'static str = "app.exit";
    pub const SUSPEND: &'static str = "app.suspend";
    pub const THINKING_CYCLE: &'static str = "app.thinking.cycle";
    pub const MODEL_CYCLE_FORWARD: &'static str = "app.model.cycleForward";
    pub const MODEL_CYCLE_BACKWARD: &'static str = "app.model.cycleBackward";
    pub const MODEL_SELECT: &'static str = "app.model.select";
    pub const TOOLS_EXPAND: &'static str = "app.tools.expand";
    pub const THINKING_TOGGLE: &'static str = "app.thinking.toggle";
    pub const SESSION_TOGGLE_NAMED_FILTER: &'static str = "app.session.toggleNamedFilter";
    pub const EDITOR_EXTERNAL: &'static str = "app.editor.external";
    pub const MESSAGE_FOLLOW_UP: &'static str = "app.message.followUp";
    pub const MESSAGE_DEQUEUE: &'static str = "app.message.dequeue";
    pub const CLIPBOARD_PASTE_IMAGE: &'static str = "app.clipboard.pasteImage";
    pub const SESSION_NEW: &'static str = "app.session.new";
    pub const SESSION_TREE: &'static str = "app.session.tree";
    pub const SESSION_FORK: &'static str = "app.session.fork";
    pub const SESSION_RESUME: &'static str = "app.session.resume";
    pub const TREE_FOLD_OR_UP: &'static str = "app.tree.foldOrUp";
    pub const TREE_UNFOLD_OR_DOWN: &'static str = "app.tree.unfoldOrDown";
    pub const TREE_EDIT_LABEL: &'static str = "app.tree.editLabel";
    pub const TREE_TOGGLE_LABEL_TIMESTAMP: &'static str = "app.tree.toggleLabelTimestamp";
    pub const SESSION_TOGGLE_PATH: &'static str = "app.session.togglePath";
    pub const SESSION_TOGGLE_SORT: &'static str = "app.session.toggleSort";
    pub const SESSION_RENAME: &'static str = "app.session.rename";
    pub const SESSION_DELETE: &'static str = "app.session.delete";
    pub const SESSION_DELETE_NONINVASIVE: &'static str = "app.session.deleteNoninvasive";
    pub const MODELS_SAVE: &'static str = "app.models.save";
    pub const MODELS_ENABLE_ALL: &'static str = "app.models.enableAll";
    pub const MODELS_CLEAR_ALL: &'static str = "app.models.clearAll";
    pub const MODELS_TOGGLE_PROVIDER: &'static str = "app.models.toggleProvider";
    pub const MODELS_REORDER_UP: &'static str = "app.models.reorderUp";
    pub const MODELS_REORDER_DOWN: &'static str = "app.models.reorderDown";
    pub const TREE_FILTER_DEFAULT: &'static str = "app.tree.filter.default";
    pub const TREE_FILTER_NO_TOOLS: &'static str = "app.tree.filter.noTools";
    pub const TREE_FILTER_USER_ONLY: &'static str = "app.tree.filter.userOnly";
    pub const TREE_FILTER_LABELED_ONLY: &'static str = "app.tree.filter.labeledOnly";
    pub const TREE_FILTER_ALL: &'static str = "app.tree.filter.all";
    pub const TREE_FILTER_CYCLE_FORWARD: &'static str = "app.tree.filter.cycleForward";
    pub const TREE_FILTER_CYCLE_BACKWARD: &'static str = "app.tree.filter.cycleBackward";
}

pub fn keybinding_name_migrations() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("cursorUp", "tui.editor.cursorUp");
    m.insert("cursorDown", "tui.editor.cursorDown");
    m.insert("cursorLeft", "tui.editor.cursorLeft");
    m.insert("cursorRight", "tui.editor.cursorRight");
    m.insert("cursorWordLeft", "tui.editor.cursorWordLeft");
    m.insert("cursorWordRight", "tui.editor.cursorWordRight");
    m.insert("cursorLineStart", "tui.editor.cursorLineStart");
    m.insert("cursorLineEnd", "tui.editor.cursorLineEnd");
    m.insert("jumpForward", "tui.editor.jumpForward");
    m.insert("jumpBackward", "tui.editor.jumpBackward");
    m.insert("pageUp", "tui.editor.pageUp");
    m.insert("pageDown", "tui.editor.pageDown");
    m.insert("deleteCharBackward", "tui.editor.deleteCharBackward");
    m.insert("deleteCharForward", "tui.editor.deleteCharForward");
    m.insert("deleteWordBackward", "tui.editor.deleteWordBackward");
    m.insert("deleteWordForward", "tui.editor.deleteWordForward");
    m.insert("deleteToLineStart", "tui.editor.deleteToLineStart");
    m.insert("deleteToLineEnd", "tui.editor.deleteToLineEnd");
    m.insert("yank", "tui.editor.yank");
    m.insert("yankPop", "tui.editor.yankPop");
    m.insert("undo", "tui.editor.undo");
    m.insert("newLine", "tui.input.newLine");
    m.insert("submit", "tui.input.submit");
    m.insert("tab", "tui.input.tab");
    m.insert("copy", "tui.input.copy");
    m.insert("selectUp", "tui.select.up");
    m.insert("selectDown", "tui.select.down");
    m.insert("selectPageUp", "tui.select.pageUp");
    m.insert("selectPageDown", "tui.select.pageDown");
    m.insert("selectConfirm", "tui.select.confirm");
    m.insert("selectCancel", "tui.select.cancel");
    m.insert("interrupt", "app.interrupt");
    m.insert("clear", "app.clear");
    m.insert("exit", "app.exit");
    m.insert("suspend", "app.suspend");
    m.insert("cycleThinkingLevel", "app.thinking.cycle");
    m.insert("cycleModelForward", "app.model.cycleForward");
    m.insert("cycleModelBackward", "app.model.cycleBackward");
    m.insert("selectModel", "app.model.select");
    m.insert("expandTools", "app.tools.expand");
    m.insert("toggleThinking", "app.thinking.toggle");
    m.insert("toggleSessionNamedFilter", "app.session.toggleNamedFilter");
    m.insert("externalEditor", "app.editor.external");
    m.insert("followUp", "app.message.followUp");
    m.insert("dequeue", "app.message.dequeue");
    m.insert("pasteImage", "app.clipboard.pasteImage");
    m.insert("newSession", "app.session.new");
    m.insert("tree", "app.session.tree");
    m.insert("fork", "app.session.fork");
    m.insert("resume", "app.session.resume");
    m.insert("treeFoldOrUp", "app.tree.foldOrUp");
    m.insert("treeUnfoldOrDown", "app.tree.unfoldOrDown");
    m.insert("treeEditLabel", "app.tree.editLabel");
    m.insert("treeToggleLabelTimestamp", "app.tree.toggleLabelTimestamp");
    m.insert("toggleSessionPath", "app.session.togglePath");
    m.insert("toggleSessionSort", "app.session.toggleSort");
    m.insert("renameSession", "app.session.rename");
    m.insert("deleteSession", "app.session.delete");
    m.insert("deleteSessionNoninvasive", "app.session.deleteNoninvasive");
    m
}

pub fn migrate_keybindings_config(raw_config: HashMap<String, serde_json::Value>) -> MigrateResult {
    let migrations = keybinding_name_migrations();
    let mut config = HashMap::new();
    let mut migrated = false;
    let default_keys = default_keybindings();

    for (key, value) in raw_config {
        let next_key = migrations
            .get(key.as_str())
            .map(|&s| s.to_string())
            .unwrap_or_else(|| key.clone());
        if next_key != key {
            migrated = true;
        }
        if key != next_key && config.contains_key(&next_key) {
            migrated = true;
            continue;
        }
        config.insert(next_key, value);
    }

    let ordered = order_keybindings_config(config, &default_keys);

    MigrateResult { config: ordered, migrated }
}

pub struct MigrateResult {
    pub config: HashMap<String, serde_json::Value>,
    pub migrated: bool,
}

fn order_keybindings_config(
    config: HashMap<String, serde_json::Value>,
    default_keys: &KeybindingDefinitions,
) -> HashMap<String, serde_json::Value> {
    let mut ordered = HashMap::new();

    for key in default_keys.keys() {
        if let Some(value) = config.get(key) {
            ordered.insert(key.clone(), value.clone());
        }
    }

    let mut extras: Vec<&String> = config.keys().filter(|k| !default_keys.contains_key(*k)).collect();
    extras.sort();
    for key in extras {
        ordered.insert(key.clone(), config[key].clone());
    }

    ordered
}

pub fn to_keybindings_config(value: serde_json::Value) -> KeybindingsConfig {
    let mut config = KeybindingsConfig::new();

    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map {
                match val {
                    serde_json::Value::String(s) => {
                        config.insert(key, KeybindingValue::Single(s));
                    }
                    serde_json::Value::Array(arr) => {
                        let keys: Vec<String> = arr
                            .into_iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                        if !keys.is_empty() {
                            config.insert(key, KeybindingValue::Multiple(keys));
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    config
}

pub fn load_raw_config(path: &std::path::Path) -> Option<HashMap<String, serde_json::Value>> {
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&content).ok()?;
    match parsed {
        serde_json::Value::Object(map) => {
            let result: HashMap<String, serde_json::Value> =
                map.into_iter().collect();
            Some(result)
        }
        _ => None,
    }
}

pub struct KeybindingsManager {
    default_definitions: KeybindingDefinitions,
    user_bindings: KeybindingsConfig,
    config_path: Option<PathBuf>,
}

impl KeybindingsManager {
    pub fn new(user_bindings: KeybindingsConfig, config_path: Option<PathBuf>) -> Self {
        Self {
            default_definitions: default_keybindings(),
            user_bindings,
            config_path,
        }
    }

    pub fn create(agent_dir: &str) -> Self {
        let config_path = std::path::Path::new(agent_dir).join("keybindings.json");
        let user_bindings = Self::load_from_file(&config_path);
        Self::new(user_bindings, Some(config_path))
    }

    pub fn reload(&mut self) {
        if let Some(ref path) = self.config_path {
            self.user_bindings = Self::load_from_file(path);
        }
    }

    pub fn get_effective_config(&self) -> &KeybindingsConfig {
        &self.user_bindings
    }

    pub fn set_user_bindings(&mut self, bindings: KeybindingsConfig) {
        self.user_bindings = bindings;
    }

    pub fn get_resolved_bindings(&self) -> KeybindingsConfig {
        self.user_bindings.clone()
    }

    fn load_from_file(path: &std::path::Path) -> KeybindingsConfig {
        let raw_config = load_raw_config(path);
        match raw_config {
            Some(raw) => {
                let migrated = migrate_keybindings_config(raw);
                to_keybindings_config(serde_json::to_value(migrated.config).unwrap_or(serde_json::Value::Null))
            }
            None => KeybindingsConfig::new(),
        }
    }
}
