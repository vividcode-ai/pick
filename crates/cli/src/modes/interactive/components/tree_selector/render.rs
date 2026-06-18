use std::collections::HashMap;
use std::collections::HashSet;

use crate::core::tools::render_utils::ToolTheme;

use super::{FlatTreeDisplayNode, TreeNodeInfo, ToolCallInfo, TreeFilterMode};

pub fn get_entry_display_text(node: &TreeNodeInfo, tool_calls: &HashMap<String, ToolCallInfo>) -> String {
    let normalize = |s: &str| -> String {
        s.replace('\n', " ").trim().to_string()
    };

    match node.entry_type.as_str() {
        "message" => {
            let role = node.role.as_deref().unwrap_or("unknown");
            match role {
                "user" => {
                    let content = node.content_text.as_deref().unwrap_or("");
                    format!("{}{}",
                        ToolTheme::fg("accent", "user: "),
                        normalize(content),
                    )
                }
                "assistant" => {
                    if let Some(ref text) = node.content_text {
                        if !text.trim().is_empty() {
                            format!("{}{}",
                                ToolTheme::fg("success", "assistant: "),
                                normalize(text),
                            )
                        } else if node.stop_reason.as_deref() == Some("aborted") {
                            format!("{}{}",
                                ToolTheme::fg("success", "assistant: "),
                                ToolTheme::fg("muted", "(aborted)"),
                            )
                        } else if let Some(ref err) = node.error_message {
                            let truncated = normalize(err);
                            let truncated = if truncated.len() > 80 { format!("{}...", &truncated[..80]) } else { truncated };
                            format!("{}{}",
                                ToolTheme::fg("success", "assistant: "),
                                ToolTheme::fg("error", &truncated),
                            )
                        } else {
                            format!("{}{}",
                                ToolTheme::fg("success", "assistant: "),
                                ToolTheme::fg("muted", "(no content)"),
                            )
                        }
                    } else {
                        format!("{}{}",
                            ToolTheme::fg("success", "assistant: "),
                            ToolTheme::fg("muted", "(no content)"),
                        )
                    }
                }
                "toolResult" => {
                    if let Some(ref id) = node.tool_call_id {
                        if let Some(tc) = tool_calls.get(id) {
                            ToolTheme::fg("muted", &format_tool_call(&tc.name, &tc.args))
                        } else {
                            ToolTheme::fg("muted", &format!("[{}]", node.tool_name.as_deref().unwrap_or("tool")))
                        }
                    } else {
                        ToolTheme::fg("muted", &format!("[{}]", node.tool_name.as_deref().unwrap_or("tool")))
                    }
                }
                "bashExecution" => {
                    let cmd = node.command.as_deref().unwrap_or("");
                    ToolTheme::fg("dim", &format!("[bash]: {}", normalize(cmd)))
                }
                _ => ToolTheme::fg("dim", &format!("[{}]", role)),
            }
        }
        "custom_message" => {
            let content = node.content_text.as_deref().unwrap_or("");
            let ct = node.custom_type.as_deref().unwrap_or("custom");
            format!("{}{}",
                ToolTheme::fg("customMessageLabel", &format!("[{}]: ", ct)),
                normalize(content),
            )
        }
        "compaction" => {
            let tokens = node.tokens_before / 1000;
            ToolTheme::fg("dim", &format!("[compaction: {}k tokens]", tokens))
        }
        "branch_summary" => {
            let summary = node.summary.as_deref().unwrap_or("");
            format!("{}{}",
                ToolTheme::fg("warning", "[branch summary]: "),
                normalize(summary),
            )
        }
        "model_change" => {
            let mid = node.model_id.as_deref().unwrap_or("?");
            ToolTheme::fg("dim", &format!("[model: {}]", mid))
        }
        "thinking_level_change" => {
            let tl = node.thinking_level.as_deref().unwrap_or("?");
            ToolTheme::fg("dim", &format!("[thinking: {}]", tl))
        }
        "custom" => {
            let ct = node.custom_type.as_deref().unwrap_or("?");
            ToolTheme::fg("dim", &format!("[custom: {}]", ct))
        }
        "label" => {
            let l = node.label.as_deref().unwrap_or("(cleared)");
            ToolTheme::fg("dim", &format!("[label: {}]", l))
        }
        "session_info" => {
            if let Some(ref name) = node.name {
                format!("{}{}{}",
                    ToolTheme::fg("dim", "[title: "),
                    ToolTheme::fg("dim", name),
                    ToolTheme::fg("dim", "]"),
                )
            } else {
                format!("{}{}{}",
                    ToolTheme::fg("dim", "[title: "),
                    ToolTheme::fg("dim", "empty"),
                    ToolTheme::fg("dim", "]"),
                )
            }
        }
        _ => String::new(),
    }
}

fn format_tool_call(name: &str, args_json: &str) -> String {
    match name {
        "read" | "write" | "edit" | "grep" | "find" | "ls" => {
            format!("[{}: {}]", name, args_json)
        }
        "bash" => {
            let truncated = if args_json.len() > 50 {
                format!("{}...", &args_json[..50])
            } else {
                args_json.to_string()
            };
            let cmd = truncated.replace('\n', " ").trim().to_string();
            ToolTheme::fg("dim", &format!("[bash]: {}", cmd))
        }
        _ => {
            let args_trunc = if args_json.len() > 40 {
                format!("{}...", &args_json[..40])
            } else {
                args_json.to_string()
            };
            ToolTheme::fg("muted", &format!("[{}: {}]", name, args_trunc))
        }
    }
}

pub fn render_tree_list(
    flat_nodes: &[FlatTreeDisplayNode],
    nodes: &[TreeNodeInfo],
    selected_index: usize,
    filter_mode: TreeFilterMode,
    show_label_timestamps: bool,
    active_path_ids: &HashSet<String>,
    folded_nodes: &HashSet<String>,
    tool_calls: &HashMap<String, ToolCallInfo>,
    width: usize,
    max_visible: usize,
) -> Vec<String> {
    let mut lines = Vec::new();

    if flat_nodes.is_empty() {
        lines.push(String::new());
        lines.push(ToolTheme::fg("muted", "  No entries found"));
        let status = get_status_labels(filter_mode, show_label_timestamps);
        lines.push(ToolTheme::fg("muted", &format!("  (0/0){}", status)));
        return lines;
    }

    let total = flat_nodes.len();
    let start = if total > max_visible {
        let half = max_visible / 2;
        if selected_index > half {
            std::cmp::min(selected_index - half, total - max_visible)
        } else {
            0
        }
    } else {
        0
    };
    let end = std::cmp::min(start + max_visible, total);

    for i in start..end {
        if let Some(flat_node) = flat_nodes.get(i) {
            let node = &nodes[flat_node.node_idx];
            let is_selected = i == selected_index;
            let is_folded = folded_nodes.contains(&node.entry_id);

            let cursor = if is_selected {
                ToolTheme::fg("accent", "› ")
            } else {
                "  ".to_string()
            };

            let display_indent = flat_node.indent;
            let prefix = build_tree_prefix_flat(flat_node, is_folded, display_indent);

            let is_on_active = active_path_ids.contains(&node.entry_id);
            let path_marker = if is_on_active {
                ToolTheme::fg("accent", "• ")
            } else {
                String::new()
            };

            let label = node.label.as_ref().map(|l| ToolTheme::fg("warning", &format!("[{}] ", l))).unwrap_or_default();
            let label_ts = if show_label_timestamps && node.label.is_some() {
                if let Some(ref ts) = node.label_timestamp {
                    format!("{} ", ToolTheme::fg("muted", &format_label_timestamp(ts)))
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            let content = get_entry_display_text(node, tool_calls);

            let mut line = format!("{}{}{}{}{}{}",
                cursor,
                ToolTheme::fg("dim", &prefix),
                label, label_ts,
                path_marker,
                content,
            );

            if is_selected {
                line = format!("\x1b[48;5;236m{}\x1b[49m", line);
            }

            let truncated = if line.len() > width {
                format!("{}…", &line[..width.saturating_sub(1)])
            } else {
                line
            };
            lines.push(truncated);
        }
    }

    let status = get_status_labels(filter_mode, show_label_timestamps);
    lines.push(ToolTheme::fg("muted", &format!("  ({}/{}){}", selected_index + 1, total, status)));

    lines
}

fn build_tree_prefix_flat(flat_node: &FlatTreeDisplayNode, is_folded: bool, display_indent: usize) -> String {
    if display_indent == 0 {
        return String::new();
    }

    let mut result = String::new();
    for _ in 0..display_indent.saturating_sub(1) {
        result.push_str("   ");
    }

    if flat_node.show_connector && !flat_node.is_virtual_root_child {
        if is_folded {
            result.push_str("⊞ ");
        } else {
            result.push_str("├─ ");
        }
    } else {
        result.push_str("  ");
    }

    result
}

fn get_status_labels(filter_mode: TreeFilterMode, show_label_timestamps: bool) -> String {
    let mut labels = String::new();
    match filter_mode {
        TreeFilterMode::NoTools => labels += " [no-tools]",
        TreeFilterMode::UserOnly => labels += " [user]",
        TreeFilterMode::LabeledOnly => labels += " [labeled]",
        TreeFilterMode::All => labels += " [all]",
        TreeFilterMode::Default => {}
    }
    if show_label_timestamps {
        labels += " [+label time]";
    }
    labels
}

fn format_label_timestamp(timestamp: &str) -> String {
    if timestamp.len() >= 16 {
        format!("{}:{}", &timestamp[11..13], &timestamp[14..16])
    } else {
        timestamp.to_string()
    }
}
