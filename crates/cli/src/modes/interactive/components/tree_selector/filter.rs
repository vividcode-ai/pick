use std::collections::HashSet;

use super::{FlatTreeDisplayNode, TreeFilterMode, TreeNodeInfo};

pub fn filter_tree_nodes(
    flat_nodes: &[FlatTreeDisplayNode],
    nodes: &[TreeNodeInfo],
    filter_mode: TreeFilterMode,
    search_query: &str,
    current_leaf_id: Option<&str>,
    folded_nodes: &HashSet<String>,
) -> Vec<FlatTreeDisplayNode> {
    let search_tokens: Vec<&str> = search_query
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .collect();

    let filtered: Vec<&FlatTreeDisplayNode> = flat_nodes
        .iter()
        .filter(|fnode| {
            let node = &nodes[fnode.node_idx];
            let is_current = current_leaf_id.is_some_and(|id| id == node.entry_id);

            if node.entry_type == "message"
                && node.role.as_deref() == Some("assistant")
                && !is_current
                && !has_text_content(node.content_text.as_deref())
            {
                let is_error = node
                    .stop_reason
                    .as_deref()
                    .is_some_and(|r| r != "stop" && r != "toolUse");
                if !is_error {
                    return false;
                }
            }

            match filter_mode {
                TreeFilterMode::UserOnly => {
                    if !(node.entry_type == "message" && node.role.as_deref() == Some("user")) {
                        return false;
                    }
                }
                TreeFilterMode::NoTools => {
                    if is_settings_entry(&node.entry_type) {
                        return false;
                    }
                    if node.entry_type == "message" && node.role.as_deref() == Some("toolResult") {
                        return false;
                    }
                }
                TreeFilterMode::LabeledOnly => {
                    if node.label.is_none() {
                        return false;
                    }
                }
                TreeFilterMode::All => {}
                TreeFilterMode::Default => {
                    if is_settings_entry(&node.entry_type) {
                        return false;
                    }
                }
            }

            if !search_tokens.is_empty() {
                let searchable = get_searchable_text(node).to_lowercase();
                for token in &search_tokens {
                    if !searchable.contains(token) {
                        return false;
                    }
                }
            }

            true
        })
        .collect();

    let skip_set: HashSet<String> = if folded_nodes.is_empty() {
        HashSet::new()
    } else {
        let mut skip = HashSet::new();
        let parent_map: std::collections::HashMap<&str, &str> = nodes
            .iter()
            .filter_map(|n| {
                n.parent_id
                    .as_ref()
                    .map(|p| (n.entry_id.as_str(), p.as_str()))
            })
            .collect();

        for fnode in flat_nodes {
            let node = &nodes[fnode.node_idx];
            let id = &node.entry_id;
            if let Some(parent) = parent_map.get(id.as_str())
                && (folded_nodes.contains(*parent) || skip.contains(*parent))
            {
                skip.insert(id.clone());
            }
        }
        skip
    };

    let mut result: Vec<FlatTreeDisplayNode> = filtered
        .into_iter()
        .filter(|fnode| !skip_set.contains(&nodes[fnode.node_idx].entry_id))
        .cloned()
        .collect();

    recalculate_visual_structure(&mut result, nodes);

    result
}

fn has_text_content(content_text: Option<&str>) -> bool {
    content_text.is_some_and(|t| !t.trim().is_empty())
}

fn is_settings_entry(entry_type: &str) -> bool {
    matches!(
        entry_type,
        "label" | "custom" | "model_change" | "thinking_level_change" | "session_info"
    )
}

fn get_searchable_text(node: &TreeNodeInfo) -> String {
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

fn recalculate_visual_structure(flat_nodes: &mut Vec<FlatTreeDisplayNode>, nodes: &[TreeNodeInfo]) {
    if flat_nodes.is_empty() {
        return;
    }

    let visible_ids: HashSet<String> = flat_nodes
        .iter()
        .map(|f| nodes[f.node_idx].entry_id.clone())
        .collect();

    let find_visible_ancestor = |node_id: &str| -> Option<String> {
        let mut current = nodes
            .iter()
            .find(|n| n.entry_id == node_id)?
            .parent_id
            .clone()?;
        loop {
            if visible_ids.contains(&current) {
                return Some(current);
            }
            current = nodes
                .iter()
                .find(|n| n.entry_id == current)?
                .parent_id
                .clone()?;
        }
    };

    let mut visible_children: std::collections::HashMap<Option<String>, Vec<String>> =
        std::collections::HashMap::new();
    visible_children.entry(None).or_default();

    for fnode in flat_nodes.iter() {
        let id = &nodes[fnode.node_idx].entry_id;
        let ancestor = find_visible_ancestor(id);
        visible_children
            .entry(ancestor.clone())
            .or_default()
            .push(id.clone());

        if ancestor.is_none() {
            visible_children.get_mut(&None).unwrap().push(id.clone());
        }
    }

    let visible_root_ids = visible_children.get(&None).cloned().unwrap_or_default();
    let multiple_roots = visible_root_ids.len() > 1;

    for fnode in flat_nodes.iter_mut() {
        let id = &nodes[fnode.node_idx].entry_id;
        let mut depth = 0usize;
        let mut current_id: Option<String> = Some(id.clone());
        while let Some(ref cid) = current_id {
            let ancestor = find_visible_ancestor(cid);
            if ancestor.is_some() {
                depth += 1;
            }
            current_id = ancestor;
        }

        fnode.indent = if multiple_roots { depth + 1 } else { depth };
        fnode.show_connector = false;
        fnode.is_last = true;
        fnode.gutters = vec![];
        fnode.is_virtual_root_child = multiple_roots;
    }
}
