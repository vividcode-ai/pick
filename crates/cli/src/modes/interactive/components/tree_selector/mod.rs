//! Tree selector component for session tree navigation

pub mod filter;
pub mod render;

use crate::core::tools::render_utils::ToolTheme;

#[derive(Debug, Clone)]
pub struct TreeNodeInfo {
    pub entry_id: String,
    pub parent_id: Option<String>,
    pub entry_type: String,
    pub role: Option<String>,
    pub content_text: Option<String>,
    pub label: Option<String>,
    pub label_timestamp: Option<String>,
    pub custom_type: Option<String>,
    pub stop_reason: Option<String>,
    pub error_message: Option<String>,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub command: Option<String>,
    pub model_id: Option<String>,
    pub thinking_level: Option<String>,
    pub summary: Option<String>,
    pub name: Option<String>,
    pub tokens_before: u64,
    pub children: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct SessionTree {
    pub nodes: Vec<TreeNodeInfo>,
    pub root_indices: Vec<usize>,
    pub current_leaf_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FlatTreeDisplayNode {
    pub node_idx: usize,
    pub indent: usize,
    pub show_connector: bool,
    pub is_last: bool,
    pub gutters: Vec<GutterInfo>,
    pub is_virtual_root_child: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct GutterInfo {
    pub position: usize,
    pub show: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TreeFilterMode {
    Default,
    NoTools,
    UserOnly,
    LabeledOnly,
    All,
}

impl TreeFilterMode {
    pub fn label(&self) -> &'static str {
        match self {
            TreeFilterMode::Default => "default",
            TreeFilterMode::NoTools => "no-tools",
            TreeFilterMode::UserOnly => "user-only",
            TreeFilterMode::LabeledOnly => "labeled-only",
            TreeFilterMode::All => "all",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolCallInfo {
    pub name: String,
    pub args: String,
}

pub fn build_active_path(
    nodes: &[TreeNodeInfo],
    leaf_id: Option<&str>,
) -> std::collections::HashSet<String> {
    let mut path = std::collections::HashSet::new();

    if let Some(leaf) = leaf_id {
        let mut current = leaf.to_string();
        loop {
            path.insert(current.clone());
            let node = nodes.iter().find(|n| n.entry_id == current);
            match node {
                Some(n) => {
                    if let Some(ref parent) = n.parent_id {
                        current = parent.clone();
                    } else {
                        break;
                    }
                }
                None => break,
            }
        }
    }

    path
}

pub fn build_tool_call_map(
    _nodes: &[TreeNodeInfo],
) -> std::collections::HashMap<String, ToolCallInfo> {
    
    std::collections::HashMap::new()
}

pub fn compute_contains_active(
    nodes: &[TreeNodeInfo],
    root_indices: &[usize],
    children: &[Vec<usize>],
    leaf_id: Option<&str>,
) -> Vec<bool> {
    let mut contains = vec![false; nodes.len()];

    fn visit(
        idx: usize,
        nodes: &[TreeNodeInfo],
        children: &[Vec<usize>],
        leaf_id: Option<&str>,
        contains: &mut [bool],
    ) -> bool {
        let mut has = leaf_id.is_some_and(|id| nodes[idx].entry_id == id);
        for &child in &children[idx] {
            if visit(child, nodes, children, leaf_id, contains) {
                has = true;
            }
        }
        contains[idx] = has;
        has
    }

    for &root in root_indices {
        visit(root, nodes, children, leaf_id, &mut contains);
    }

    contains
}

pub fn has_text_content(content_text: Option<&str>) -> bool {
    content_text.is_some_and(|t| !t.trim().is_empty())
}

pub fn is_settings_entry(entry_type: &str) -> bool {
    matches!(
        entry_type,
        "label" | "custom" | "model_change" | "thinking_level_change" | "session_info"
    )
}

pub fn get_searchable_text(node: &TreeNodeInfo) -> String {
    let mut parts = Vec::new();
    if let Some(ref label) = node.label {
        parts.push(label.clone());
    }
    match node.entry_type.as_str() {
        "message" => {
            if let Some(ref role) = node.role {
                parts.push(role.clone());
            }
            if let Some(ref content) = node.content_text {
                parts.push(content.clone());
            }
            if let Some(ref cmd) = node.command {
                parts.push(cmd.clone());
            }
        }
        "custom_message" => {
            if let Some(ref ct) = node.custom_type {
                parts.push(ct.clone());
            }
            if let Some(ref content) = node.content_text {
                parts.push(content.clone());
            }
        }
        "compaction" => parts.push("compaction".to_string()),
        "branch_summary" => {
            parts.push("branch summary".to_string());
            if let Some(ref s) = node.summary {
                parts.push(s.clone());
            }
        }
        "session_info" => {
            parts.push("title".to_string());
            if let Some(ref name) = node.name {
                parts.push(name.clone());
            }
        }
        "model_change" => {
            parts.push("model".to_string());
            if let Some(ref m) = node.model_id {
                parts.push(m.clone());
            }
        }
        "thinking_level_change" => {
            parts.push("thinking".to_string());
            if let Some(ref tl) = node.thinking_level {
                parts.push(tl.clone());
            }
        }
        "custom" => {
            parts.push("custom".to_string());
            if let Some(ref ct) = node.custom_type {
                parts.push(ct.clone());
            }
        }
        "label" => {
            parts.push("label".to_string());
            if let Some(ref l) = node.label {
                parts.push(l.clone());
            }
        }
        _ => {}
    }
    parts.join(" ")
}

pub fn format_tool_call(name: &str, args_json: &str) -> String {
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

pub fn render_tree_selector_header(
    _filter_mode: TreeFilterMode,
    _show_label_timestamps: bool,
    _width: usize,
) -> Vec<String> {
    let filter_keys = format!(
        "{}D{}/{}N{}/{}U{}/{}L{}/{}A{}",
        ToolTheme::fg("accent", ""),
        ToolTheme::fg("muted", ""),
        ToolTheme::fg("accent", ""),
        ToolTheme::fg("muted", ""),
        ToolTheme::fg("accent", ""),
        ToolTheme::fg("muted", ""),
        ToolTheme::fg("accent", ""),
        ToolTheme::fg("muted", ""),
        ToolTheme::fg("accent", ""),
        ToolTheme::fg("muted", ""),
    );

    let hints = format!(
        "  ↑/↓: move. ←/→: page. F/uF: fold/branch. L: label. {}: filters (F/f cycle). T: label time",
        filter_keys
    );

    vec![
        ToolTheme::bold("  Session Tree"),
        ToolTheme::fg("muted", &hints),
    ]
}

pub fn render_tree_search_line(query: &str, _width: usize) -> Vec<String> {
    if query.is_empty() {
        vec![format!("  {}", ToolTheme::fg("muted", "Type to search:"))]
    } else {
        vec![format!(
            "  {}Type to search: {}",
            ToolTheme::fg("muted", ""),
            ToolTheme::fg("accent", query),
        )]
    }
}

pub fn render_label_input(current_label: Option<&str>, _width: usize) -> Vec<String> {
    let indent = "  ";
    let lines = vec![
        format!(
            "{}{}",
            indent,
            ToolTheme::fg("muted", "Label (empty to remove):")
        ),
        format!("{}{}", indent, current_label.unwrap_or("")),
        format!(
            "{}{}",
            indent,
            ToolTheme::fg("muted", "Enter: save · Esc: cancel")
        ),
    ];
    lines
}

pub fn render_tree_selector(
    flat_nodes: &[FlatTreeDisplayNode],
    nodes: &[TreeNodeInfo],
    selected_index: usize,
    search_query: &str,
    filter_mode: TreeFilterMode,
    show_label_timestamps: bool,
    active_path_ids: &std::collections::HashSet<String>,
    folded_nodes: &std::collections::HashSet<String>,
    tool_calls: &std::collections::HashMap<String, ToolCallInfo>,
    width: usize,
    max_visible: usize,
    editing_label: bool,
    label_edit_value: Option<&str>,
) -> Vec<String> {
    let mut lines = Vec::new();
    let border = "─".repeat(std::cmp::max(1, width));

    lines.push(ToolTheme::fg("accent", &border));
    lines.push(String::new());

    lines.extend(render_tree_selector_header(
        filter_mode,
        show_label_timestamps,
        width,
    ));
    lines.push(String::new());

    lines.extend(render_tree_search_line(search_query, width));

    lines.push(ToolTheme::fg("accent", &border));
    lines.push(String::new());

    if editing_label {
        lines.extend(render_label_input(label_edit_value, width));
    } else {
        lines.extend(render_tree_list(
            flat_nodes,
            nodes,
            selected_index,
            filter_mode,
            show_label_timestamps,
            active_path_ids,
            folded_nodes,
            tool_calls,
            width,
            max_visible,
        ));
    }

    lines.push(String::new());
    lines.push(ToolTheme::fg("accent", &border));
    lines
}

pub use render::render_tree_list;
