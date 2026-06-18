//! Permission audit trail — records all permission decisions for inspection.
//!
//! Each decision (allow/deny) is logged with the tool name, permission key,
//! matched rule/pattern, source layer, and a human-readable reason.
//! The trail is stored in-memory and can be optionally persisted to a JSONL file.

use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditDecision {
    Allow,
    Deny,
    Ask,
}

impl std::fmt::Display for AuditDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditDecision::Allow => write!(f, "allow"),
            AuditDecision::Deny => write!(f, "deny"),
            AuditDecision::Ask => write!(f, "ask"),
        }
    }
}

/// Which permission layer made the decision.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditLayer {
    PreHook,
    ModeRuleset,
    PermissionHook,
    ExecPolicy,
    FileSystemPolicy,
    NetworkPolicy,
    ExternalDir,
    Sandbox,
    Guardian,
}

impl std::fmt::Display for AuditLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditLayer::PreHook => write!(f, "pre_hook"),
            AuditLayer::ModeRuleset => write!(f, "mode_ruleset"),
            AuditLayer::PermissionHook => write!(f, "permission_hook"),
            AuditLayer::ExecPolicy => write!(f, "exec_policy"),
            AuditLayer::FileSystemPolicy => write!(f, "fs_policy"),
            AuditLayer::NetworkPolicy => write!(f, "network_policy"),
            AuditLayer::ExternalDir => write!(f, "external_dir"),
            AuditLayer::Sandbox => write!(f, "sandbox"),
            AuditLayer::Guardian => write!(f, "guardian"),
        }
    }
}

/// A single permission audit event.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditEvent {
    pub timestamp: i64,
    pub tool_name: String,
    pub permission_key: String,
    pub target: String,
    pub decision: AuditDecision,
    pub layer: AuditLayer,
    pub reason: String,
    pub matched_rule: Option<String>,
}

/// In-memory audit trail, optionally persisted to a JSONL file.
pub struct AuditTrail {
    events: Mutex<Vec<AuditEvent>>,
    file_path: Option<std::path::PathBuf>,
    max_events: usize,
}

impl AuditTrail {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::with_capacity(1024)),
            file_path: None,
            max_events: 100_000,
        }
    }

    pub fn with_file(mut self, path: &Path) -> Self {
        self.file_path = Some(path.to_path_buf());
        self
    }

    pub fn with_max_events(mut self, max: usize) -> Self {
        self.max_events = max;
        self
    }

    /// Record a permission decision.
    pub fn record(
        &self,
        tool_name: &str,
        permission_key: &str,
        target: &str,
        decision: AuditDecision,
        layer: AuditLayer,
        reason: &str,
        matched_rule: Option<&str>,
    ) {
        let event = AuditEvent {
            timestamp: chrono::Utc::now().timestamp_millis(),
            tool_name: tool_name.to_string(),
            permission_key: permission_key.to_string(),
            target: target.to_string(),
            decision,
            layer,
            reason: reason.to_string(),
            matched_rule: matched_rule.map(|r| r.to_string()),
        };

        let mut events = self.events.lock().unwrap();
        let json = serde_json::to_string(&event).unwrap_or_default();

        // In-memory ring buffer
        if events.len() >= self.max_events {
            events.remove(0);
        }
        events.push(event);

        // Persistent file logging
        if let Some(ref path) = self.file_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            use std::io::Write;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = writeln!(file, "{}", json);
            }
        }
    }

    /// Return all events matching the given filter predicates.
    pub fn query<F>(&self, filter: F) -> Vec<AuditEvent>
    where
        F: Fn(&AuditEvent) -> bool,
    {
        self.events.lock().unwrap().iter().filter(|e| filter(e)).cloned().collect()
    }

    /// Return recent events (last N).
    pub fn recent(&self, n: usize) -> Vec<AuditEvent> {
        let events = self.events.lock().unwrap();
        let len = events.len();
        events.iter().skip(len.saturating_sub(n)).cloned().collect()
    }

    /// Return all events.
    pub fn all(&self) -> Vec<AuditEvent> {
        self.events.lock().unwrap().clone()
    }

    /// Clear all events.
    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

impl Default for AuditTrail {
    fn default() -> Self {
        Self::new()
    }
}
