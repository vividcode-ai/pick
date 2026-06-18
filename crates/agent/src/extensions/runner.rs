//! Extension runner - manages extension lifecycle and event dispatch

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::types::{
    BeforeAgentStartEvent, Extension, ExtensionEvent, ExtensionShortcut, InputEvent,
    InputEventResult, MessageEndEvent, RegisteredTool, ResolvedCommand, ResourcesDiscoverEvent,
    ResourcesDiscoverResult, ToolCallEvent, ToolCallEventResult, ToolResultEvent, UserBashEvent,
    UserBashEventResult,
};

/// Combined result from before_agent_start handlers
#[derive(Debug, Default)]
pub struct BeforeAgentStartCombinedResult {
    pub system_prompt: Option<String>,
}

/// Error from extension execution
#[derive(Debug, Clone)]
pub struct ExtensionError {
    pub extension_path: String,
    pub event: String,
    pub error: String,
}

/// Extension runner
pub struct ExtensionRunner {
    extensions: Arc<RwLock<Vec<Extension>>>,
    error_listeners: Arc<RwLock<Vec<Box<dyn Fn(&ExtensionError) + Send + Sync>>>>,
}

impl ExtensionRunner {
    pub fn new(extensions: Vec<Extension>) -> Self {
        Self {
            extensions: Arc::new(RwLock::new(extensions)),
            error_listeners: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register an error listener
    pub fn on_error<F>(&self, listener: F)
    where
        F: Fn(&ExtensionError) + Send + Sync + 'static,
    {
        if let Ok(mut listeners) = self.error_listeners.write() {
            listeners.push(Box::new(listener));
        }
    }

    fn emit_error(&self, error: ExtensionError) {
        if let Ok(listeners) = self.error_listeners.read() {
            for listener in listeners.iter() {
                listener(&error);
            }
        }
    }

    /// Check if there are any handlers for an event type
    pub fn has_handlers(&self, event_type: &str) -> bool {
        if let Ok(extensions) = self.extensions.read() {
            for ext in extensions.iter() {
                if let Some(handlers) = ext.handlers.get(event_type) {
                    if !handlers.is_empty() {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Get all registered tools (first registration per name wins)
    pub fn get_all_registered_tools(&self) -> Vec<RegisteredTool> {
        let mut tools_by_name = HashMap::new();
        if let Ok(extensions) = self.extensions.read() {
            for ext in extensions.iter() {
                for (name, tool) in &ext.tools {
                    if !tools_by_name.contains_key(name) {
                        tools_by_name.insert(name.clone(), tool.clone());
                    }
                }
            }
        }
        tools_by_name.into_values().collect()
    }

    /// Get a tool definition by name
    pub fn get_tool_definition(
        &self,
        tool_name: &str,
    ) -> Option<crate::extensions::types::ToolDefinition> {
        if let Ok(extensions) = self.extensions.read() {
            for ext in extensions.iter() {
                if let Some(tool) = ext.tools.get(tool_name) {
                    return Some(tool.definition.clone());
                }
            }
        }
        None
    }

    /// Get all registered commands
    pub fn get_registered_commands(&self) -> Vec<ResolvedCommand> {
        let mut commands = Vec::new();
        let mut counts = HashMap::new();

        if let Ok(extensions) = self.extensions.read() {
            for ext in extensions.iter() {
                for cmd in ext.commands.values() {
                    commands.push(cmd.clone());
                    *counts.entry(cmd.name.clone()).or_insert(0) += 1;
                }
            }
        }

        let mut seen = HashMap::new();
        let mut taken = std::collections::HashSet::new();
        let mut resolved = Vec::new();

        for cmd in commands {
            let count = counts.get(&cmd.name).copied().unwrap_or(1);
            let occurrence = seen.get(&cmd.name).copied().unwrap_or(0) + 1;
            seen.insert(cmd.name.clone(), occurrence);

            let invocation_name = if count > 1 {
                format!("{}:{}", cmd.name, occurrence)
            } else {
                cmd.name.clone()
            };

            let invocation_name = if taken.contains(&invocation_name) {
                let mut suffix = occurrence;
                loop {
                    suffix += 1;
                    let candidate = format!("{}:{}", cmd.name, suffix);
                    if !taken.contains(&candidate) {
                        break candidate;
                    }
                }
            } else {
                invocation_name
            };

            taken.insert(invocation_name.clone());
            resolved.push(ResolvedCommand {
                name: cmd.name,
                invocation_name,
                description: cmd.description,
                extension_path: cmd.extension_path,
            });
        }

        resolved
    }

    /// Emit an event to all extensions
    pub fn emit(&self, event: &ExtensionEvent) {
        let event_type = event.event_type();
        if let Ok(extensions) = self.extensions.read() {
            for ext in extensions.iter() {
                if let Some(handlers) = ext.handlers.get(event_type) {
                    for handler in handlers {
                        if let Err(e) = handler(event) {
                            self.emit_error(ExtensionError {
                                extension_path: ext.path.clone(),
                                event: event_type.to_string(),
                                error: e,
                            });
                        }
                    }
                }
            }
        }
    }

    /// Emit message_end event with message replacement support
    pub fn emit_message_end(&self, event: &MessageEndEvent) -> Option<serde_json::Value> {
        let mut current_message = event.message.clone();
        let mut modified = false;
        if let Ok(extensions) = self.extensions.read() {
            for ext in extensions.iter() {
                if let Some(handlers) = ext.handlers.get("message_end") {
                    for handler in handlers {
                        let current_event = MessageEndEvent {
                            message: current_message.clone(),
                        };
                        let wrapper_event = ExtensionEvent::MessageEnd(current_event);
                        match handler(&wrapper_event) {
                            Ok(Some(result)) => {
                                if let Some(msg) = result.get("message") {
                                    current_message = msg.clone();
                                    modified = true;
                                }
                            }
                            Err(e) => {
                                self.emit_error(ExtensionError {
                                    extension_path: ext.path.clone(),
                                    event: "message_end".to_string(),
                                    error: e,
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        if modified {
            Some(current_message)
        } else {
            None
        }
    }

    /// Emit tool_result event with result modification support
    pub fn emit_tool_result(&self, event: &ToolResultEvent) -> Option<serde_json::Value> {
        let mut current_event = event.clone();
        let mut modified = false;
        if let Ok(extensions) = self.extensions.read() {
            for ext in extensions.iter() {
                if let Some(handlers) = ext.handlers.get("tool_result") {
                    for handler in handlers {
                        let wrapper_event = ExtensionEvent::ToolResult(current_event.clone());
                        match handler(&wrapper_event) {
                            Ok(Some(result)) => {
                                if let Some(content) = result.get("content") {
                                    current_event.content =
                                        serde_json::from_value(content.clone()).unwrap_or_default();
                                    modified = true;
                                }
                                if let Some(is_error) =
                                    result.get("is_error").and_then(|v| v.as_bool())
                                {
                                    current_event.is_error = is_error;
                                    modified = true;
                                }
                            }
                            Err(e) => {
                                self.emit_error(ExtensionError {
                                    extension_path: ext.path.clone(),
                                    event: "tool_result".to_string(),
                                    error: e,
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        if modified {
            Some(serde_json::to_value(&current_event).unwrap_or_default())
        } else {
            None
        }
    }

    /// Emit tool_call event with blocking support
    pub fn emit_tool_call(&self, event: &ToolCallEvent) -> ToolCallEventResult {
        if let Ok(extensions) = self.extensions.read() {
            for ext in extensions.iter() {
                if let Some(handlers) = ext.handlers.get("tool_call") {
                    for handler in handlers {
                        let wrapper_event = ExtensionEvent::ToolCall(event.clone());
                        match handler(&wrapper_event) {
                            Ok(Some(result)) => {
                                let block = result
                                    .get("block")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false);
                                if block {
                                    return ToolCallEventResult {
                                        block: true,
                                        reason: result
                                            .get("reason")
                                            .and_then(|v| v.as_str())
                                            .map(String::from),
                                    };
                                }
                            }
                            Err(e) => {
                                self.emit_error(ExtensionError {
                                    extension_path: ext.path.clone(),
                                    event: "tool_call".to_string(),
                                    error: e,
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        ToolCallEventResult {
            block: false,
            reason: None,
        }
    }

    /// Execute a tool through an extension's tool_call handler.
    /// Returns `Some(serde_json::Value)` with the full result if an extension handled the tool.
    pub fn execute_extension_tool(
        &self,
        tool_call_id: &str,
        tool_name: &str,
        input: serde_json::Value,
    ) -> Option<serde_json::Value> {
        let event = ToolCallEvent {
            tool_call_id: tool_call_id.to_string(),
            tool_name: tool_name.to_string(),
            input,
        };

        if let Ok(extensions) = self.extensions.read() {
            for ext in extensions.iter() {
                if let Some(handlers) = ext.handlers.get("tool_call") {
                    for handler in handlers {
                        let wrapper = ExtensionEvent::ToolCall(event.clone());
                        match handler(&wrapper) {
                            Ok(Some(result)) => return Some(result),
                            Ok(None) => continue,
                            Err(e) => {
                                tracing::warn!("Extension tool_call handler error: {}", e);
                                continue;
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Emit user_bash event with result support
    pub fn emit_user_bash(&self, event: &UserBashEvent) -> Option<UserBashEventResult> {
        if let Ok(extensions) = self.extensions.read() {
            for ext in extensions.iter() {
                if let Some(handlers) = ext.handlers.get("user_bash") {
                    for handler in handlers {
                        let wrapper_event = ExtensionEvent::UserBash(event.clone());
                        match handler(&wrapper_event) {
                            Ok(Some(_)) => return None,
                            Err(e) => {
                                self.emit_error(ExtensionError {
                                    extension_path: ext.path.clone(),
                                    event: "user_bash".to_string(),
                                    error: e,
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        None
    }

    /// Emit input event with transform support
    pub fn emit_input(&self, event: &InputEvent) -> InputEventResult {
        let mut current_text = event.text.clone();
        if let Ok(extensions) = self.extensions.read() {
            for ext in extensions.iter() {
                if let Some(handlers) = ext.handlers.get("input") {
                    for handler in handlers {
                        let current_event = InputEvent {
                            text: current_text.clone(),
                            source: event.source,
                        };
                        let wrapper_event = ExtensionEvent::Input(current_event);
                        match handler(&wrapper_event) {
                            Ok(Some(result)) => {
                                let action = result.get("action").and_then(|v| v.as_str());
                                match action {
                                    Some("handled") => return InputEventResult::Handled,
                                    Some("transform") => {
                                        if let Some(text) =
                                            result.get("text").and_then(|v| v.as_str())
                                        {
                                            current_text = text.to_string();
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            Err(e) => {
                                self.emit_error(ExtensionError {
                                    extension_path: ext.path.clone(),
                                    event: "input".to_string(),
                                    error: e,
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        if current_text != event.text {
            InputEventResult::Transform { text: current_text }
        } else {
            InputEventResult::Continue
        }
    }

    /// Emit before_agent_start event with system prompt modification
    pub fn emit_before_agent_start(
        &self,
        event: &BeforeAgentStartEvent,
    ) -> Option<BeforeAgentStartCombinedResult> {
        let mut current_system_prompt = event.system_prompt.clone();
        let mut system_prompt_modified = false;
        if let Ok(extensions) = self.extensions.read() {
            for ext in extensions.iter() {
                if let Some(handlers) = ext.handlers.get("before_agent_start") {
                    for handler in handlers {
                        let current_event = BeforeAgentStartEvent {
                            prompt: event.prompt.clone(),
                            system_prompt: current_system_prompt.clone(),
                        };
                        let wrapper_event = ExtensionEvent::BeforeAgentStart(current_event);
                        match handler(&wrapper_event) {
                            Ok(Some(result)) => {
                                if let Some(sp) = result
                                    .get("systemPrompt")
                                    .or(result.get("system_prompt"))
                                    .and_then(|v| v.as_str())
                                {
                                    current_system_prompt = sp.to_string();
                                    system_prompt_modified = true;
                                }
                            }
                            Err(e) => {
                                self.emit_error(ExtensionError {
                                    extension_path: ext.path.clone(),
                                    event: "before_agent_start".to_string(),
                                    error: e,
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        if system_prompt_modified {
            Some(BeforeAgentStartCombinedResult {
                system_prompt: Some(current_system_prompt),
            })
        } else {
            None
        }
    }

    /// Emit resources_discover event
    pub fn emit_resources_discover(&self, cwd: &str, reason: &str) -> ResourcesDiscoverResult {
        let mut result = ResourcesDiscoverResult::default();
        let reason_enum = match reason {
            "reload" => super::types::ResourcesDiscoverReason::Reload,
            _ => super::types::ResourcesDiscoverReason::Startup,
        };
        if let Ok(extensions) = self.extensions.read() {
            for ext in extensions.iter() {
                if let Some(handlers) = ext.handlers.get("resources_discover") {
                    for handler in handlers {
                        let event = ResourcesDiscoverEvent {
                            cwd: cwd.to_string(),
                            reason: reason_enum,
                        };
                        let wrapper_event = ExtensionEvent::ResourcesDiscover(event);
                        match handler(&wrapper_event) {
                            Ok(Some(r)) => {
                                if let Some(paths) = r.get("skill_paths").and_then(|v| v.as_array())
                                {
                                    for p in paths {
                                        if let Some(s) = p.as_str() {
                                            result.skill_paths.push(s.to_string());
                                        }
                                    }
                                }
                                if let Some(paths) =
                                    r.get("prompt_paths").and_then(|v| v.as_array())
                                {
                                    for p in paths {
                                        if let Some(s) = p.as_str() {
                                            result.prompt_paths.push(s.to_string());
                                        }
                                    }
                                }
                                if let Some(paths) = r.get("theme_paths").and_then(|v| v.as_array())
                                {
                                    for p in paths {
                                        if let Some(s) = p.as_str() {
                                            result.theme_paths.push(s.to_string());
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                self.emit_error(ExtensionError {
                                    extension_path: ext.path.clone(),
                                    event: "resources_discover".to_string(),
                                    error: e,
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        result
    }

    /// Get all extension shortcuts
    pub fn get_shortcuts(&self) -> Vec<ExtensionShortcut> {
        let mut shortcuts = Vec::new();
        if let Ok(extensions) = self.extensions.read() {
            for ext in extensions.iter() {
                for shortcut in ext.shortcuts.values() {
                    shortcuts.push(shortcut.clone());
                }
            }
        }
        shortcuts
    }

    /// Check if UI is available (any extension with UI handlers)
    pub fn has_ui(&self) -> bool {
        false
    }

    /// Get extension paths
    pub fn get_extension_paths(&self) -> Vec<String> {
        if let Ok(extensions) = self.extensions.read() {
            extensions.iter().map(|e| e.path.clone()).collect()
        } else {
            vec![]
        }
    }
}
