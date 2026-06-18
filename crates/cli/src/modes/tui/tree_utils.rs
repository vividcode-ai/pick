use pick_agent::session::SessionEntryKind;

/// Get kind string for a session entry
pub(crate) fn entry_kind_str(entry: &pick_agent::session::SessionEntry) -> String {
    match &entry.kind {
        SessionEntryKind::Message(m) => match m.role.as_str() {
            "user" => "user",
            "tool_result" => "tool_result",
            "assistant" => "assistant",
            _ => "message",
        }
        .to_string(),
        SessionEntryKind::Compaction(_) => "compaction".to_string(),
        SessionEntryKind::BranchSummary(_) => "branch_summary".to_string(),
        SessionEntryKind::ModelChange(_) => "model_change".to_string(),
        SessionEntryKind::ThinkingLevelChange(_) => "thinking_level_change".to_string(),
        SessionEntryKind::Custom(_) => "custom".to_string(),
        SessionEntryKind::SessionInfo(_) => "session_info".to_string(),
        SessionEntryKind::LeafChange(_) => "leaf_change".to_string(),
        SessionEntryKind::Label(_) => "label".to_string(),
        SessionEntryKind::AgentModeChange(_) => "agent_mode_change".to_string(),
        SessionEntryKind::TodoUpdate(_) => "todo_update".to_string(),
        SessionEntryKind::Goal(_) => "goal".to_string(),
    }
}

/// Build display label for a session entry
pub(crate) fn entry_label(entry: &pick_agent::session::SessionEntry) -> String {
    match &entry.kind {
        SessionEntryKind::Message(m) => {
            let role_display = match m.role.as_str() {
                "user" => "\x1b[1muser\x1b[0m",
                "tool_result" => "\x1b[2mtool\x1b[0m",
                "assistant" => "\x1b[32massistant\x1b[0m",
                _ => &m.role,
            };
            let preview = match &m.content {
                serde_json::Value::String(s) => {
                    let cleaned = s.replace('\n', " ").chars().take(60).collect::<String>();
                    if cleaned.len() >= 60 {
                        format!("{}...", cleaned)
                    } else {
                        cleaned
                    }
                }
                serde_json::Value::Array(arr) => {
                    let texts: Vec<String> = arr
                        .iter()
                        .filter_map(|c| {
                            c.get("text").and_then(|t| t.as_str()).map(|s| {
                                s.replace('\n', " ").chars().take(60).collect::<String>()
                            })
                        })
                        .collect();
                    if texts.is_empty() {
                        format!("[{} blocks]", arr.len())
                    } else {
                        texts.join(" | ")
                    }
                }
                _ => String::new(),
            };
            if preview.is_empty() {
                format!("[{}]", role_display)
            } else {
                format!("[{}] {}", role_display, preview)
            }
        }
        SessionEntryKind::Compaction(c) => format!(
            "\x1b[33m[compaction]\x1b[0m {}",
            c.summary.chars().take(60).collect::<String>()
        ),
        SessionEntryKind::BranchSummary(b) => format!(
            "\x1b[33m[branch]\x1b[0m {}",
            b.summary.chars().take(60).collect::<String>()
        ),
        SessionEntryKind::ModelChange(mc) => {
            format!("\x1b[2m[model: {} \u{2192} {}]\x1b[0m", mc.from, mc.to)
        }
        SessionEntryKind::ThinkingLevelChange(tc) => {
            format!("\x1b[2m[thinking: {} \u{2192} {}]\x1b[0m", tc.from, tc.to)
        }
        SessionEntryKind::Custom(c) => format!("\x1b[2m[custom: {}]\x1b[0m", c.kind),
        SessionEntryKind::SessionInfo(i) => format!("\x1b[34m[info]\x1b[0m name: {}", i.name),
        SessionEntryKind::LeafChange(l) => format!(
            "\x1b[2m[leaf]\x1b[0m {} \u{2192} {}",
            l.from.as_deref().unwrap_or("(root)"),
            l.to.chars().take(8).collect::<String>()
        ),
        SessionEntryKind::Label(lb) => format!(
            "\x1b[35m[label]\x1b[0m {}: {}",
            lb.target_id.chars().take(8).collect::<String>(),
            lb.label.as_deref().unwrap_or("(cleared)")
        ),
        SessionEntryKind::AgentModeChange(amc) => {
            format!("\x1b[36m[mode]\x1b[0m {} \u{2192} {}", amc.from, amc.to)
        }
        SessionEntryKind::TodoUpdate(t) => {
            format!("\x1b[33m[todo]\x1b[0m {} tasks", t.todos.len())
        }
        SessionEntryKind::Goal(g) => format!(
            "\x1b[35m[goal]\x1b[0m {}",
            g.objective.chars().take(40).collect::<String>()
        ),
    }
}
