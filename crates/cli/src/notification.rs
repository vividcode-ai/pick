//! System notification observer for desktop alerts when the agent needs user
//! input (permission approval or question answers).

use notify_rust::{Notification, Timeout};
use pick_agent::core::hooks::{ToolEvent, ToolEventObserver, WaitingKind};

/// Observer that sends a desktop notification whenever the system is waiting
/// for user input (permission approval or question answer).
///
/// The notification auto-closes after 10 seconds.  Clicking it brings the
/// terminal window to focus, where the TUI already has the relevant dialog
/// waiting.
pub(crate) struct SystemNotificationObserver;

#[async_trait::async_trait]
impl ToolEventObserver for SystemNotificationObserver {
    fn name(&self) -> &str {
        "system-notification"
    }

    async fn on_tool_event(&self, event: &ToolEvent) {
        match event {
            ToolEvent::WaitingForUser { kind, summary, .. } => {
                let (title, body) = match kind {
                    WaitingKind::Permission { permission } => {
                        let perm_desc = permission_description(permission);
                        (format!("🔒 Pick — {}", perm_desc), truncate(summary, 120))
                    }
                    WaitingKind::Question { header, .. } => {
                        ("💬 Pick — Question tool".to_string(), header.clone())
                    }
                };

                let _ = Notification::new()
                    .summary(&title)
                    .body(&body)
                    .appname("Pick")
                    .timeout(Timeout::Milliseconds(10000))
                    .show();
            }
            _ => {}
        }
    }
}

/// Map permission key to a user-friendly description.
fn permission_description(key: &str) -> String {
    match key {
        "bash" => "Command Execution Requested".to_string(),
        "read" => "File Read Requested".to_string(),
        "write" | "edit" => "File Write Requested".to_string(),
        "network" | "webfetch" => "Network Access Requested".to_string(),
        "subagent" => "Subagent Spawn Requested".to_string(),
        "extension" => "Extension Execution Requested".to_string(),
        other => format!("Permission Required ({})", other),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut t = s[..max.saturating_sub(3)].to_string();
        t.push_str("...");
        t
    }
}
