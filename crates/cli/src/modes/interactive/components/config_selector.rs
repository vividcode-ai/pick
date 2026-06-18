//! Config selector component for resource configuration

use crate::core::tools::render_utils::ToolTheme;

/// Resource type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResourceType {
    Extensions,
    Skills,
    Prompts,
    Themes,
}

impl ResourceType {
    pub fn label(&self) -> &'static str {
        match self {
            ResourceType::Extensions => "Extensions",
            ResourceType::Skills => "Skills",
            ResourceType::Prompts => "Prompts",
            ResourceType::Themes => "Themes",
        }
    }
}

/// Resource metadata
#[derive(Debug, Clone)]
pub struct PathMetadata {
    pub origin: String,
    pub scope: String,
    pub source: String,
    pub base_dir: Option<String>,
}

/// Resource item for display
#[derive(Debug, Clone)]
pub struct ResourceItem {
    pub path: String,
    pub enabled: bool,
    pub metadata: PathMetadata,
    pub resource_type: ResourceType,
    pub display_name: String,
}

/// Resource subgroup
#[derive(Debug, Clone)]
pub struct ResourceSubgroup {
    pub resource_type: ResourceType,
    pub label: String,
    pub items: Vec<ResourceItem>,
}

/// Resource group
#[derive(Debug, Clone)]
pub struct ResourceGroup {
    pub key: String,
    pub label: String,
    pub scope: String,
    pub origin: String,
    pub source: String,
    pub subgroups: Vec<ResourceSubgroup>,
}

/// Flat entry for display
#[derive(Debug, Clone)]
pub enum FlatEntry {
    Group(ResourceGroup),
    Subgroup(ResourceSubgroup),
    Item(ResourceItem),
}

fn format_base_dir(base_dir: &str) -> String {
    let home = if cfg!(windows) {
        std::env::var("USERPROFILE").unwrap_or_else(|_| "~".to_string())
    } else {
        std::env::var("HOME").unwrap_or_else(|_| "~".to_string())
    };

    if base_dir == home {
        return "~/".to_string();
    }
    if let Some(rest) = base_dir.strip_prefix(&home) {
        let normalized = rest.replace('\\', "/");
        return format!("~{}/", normalized.trim_start_matches('/'));
    }
    let normalized = base_dir.replace('\\', "/");
    if normalized.ends_with('/') {
        normalized
    } else {
        format!("{}/", normalized)
    }
}

fn get_group_label(metadata: &PathMetadata) -> String {
    if metadata.origin == "package" {
        return format!("{} ({})", metadata.source, metadata.scope);
    }
    // Top-level resources
    if metadata.source == "auto" {
        if let Some(ref base) = metadata.base_dir {
            if metadata.scope == "user" {
                return format!("User ({})", format_base_dir(base));
            }
            return format!("Project ({})", format_base_dir(base));
        }
        if metadata.scope == "user" {
            return "User (~/.pick/agent/)".to_string();
        }
        return "Project (.pick/)".to_string();
    }
    if metadata.scope == "user" {
        "User settings".to_string()
    } else {
        "Project settings".to_string()
    }
}

/// Build groups from resources
pub fn build_resource_groups(
    extensions: &[ResourceItem],
    skills: &[ResourceItem],
    prompts: &[ResourceItem],
    themes: &[ResourceItem],
) -> Vec<ResourceGroup> {
    use std::collections::HashMap;

    let mut group_map: HashMap<String, ResourceGroup> = HashMap::new();

    let add_to_group = |group_map: &mut HashMap<String, ResourceGroup>,
                        resources: &[ResourceItem],
                        resource_type: ResourceType| {
        for res in resources {
            let group_key = format!(
                "{}:{}:{}:{}",
                res.metadata.origin,
                res.metadata.scope,
                res.metadata.source,
                res.metadata.base_dir.as_deref().unwrap_or("")
            );

            let group = group_map
                .entry(group_key.clone())
                .or_insert_with(|| ResourceGroup {
                    key: group_key,
                    label: get_group_label(&res.metadata),
                    scope: res.metadata.scope.clone(),
                    origin: res.metadata.origin.clone(),
                    source: res.metadata.source.clone(),
                    subgroups: Vec::new(),
                });

            let subgroup_key = resource_type;
            if !group
                .subgroups
                .iter()
                .any(|sg| sg.resource_type == subgroup_key)
            {
                group.subgroups.push(ResourceSubgroup {
                    resource_type,
                    label: resource_type.label().to_string(),
                    items: Vec::new(),
                });
            }
            if let Some(subgroup) = group
                .subgroups
                .iter_mut()
                .find(|sg| sg.resource_type == subgroup_key)
            {
                subgroup.items.push(res.clone());
            }
        }
    };

    add_to_group(&mut group_map, extensions, ResourceType::Extensions);
    add_to_group(&mut group_map, skills, ResourceType::Skills);
    add_to_group(&mut group_map, prompts, ResourceType::Prompts);
    add_to_group(&mut group_map, themes, ResourceType::Themes);

    // Sort groups
    let mut groups: Vec<ResourceGroup> = group_map.into_values().collect();
    groups.sort_by(|a, b| {
        if a.origin != b.origin {
            return if a.origin == "package" {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        if a.scope != b.scope {
            return if a.scope == "user" {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        a.source.cmp(&b.source)
    });

    // Sort subgroups and items
    let type_order = |rt: &ResourceType| -> u8 {
        match rt {
            ResourceType::Extensions => 0,
            ResourceType::Skills => 1,
            ResourceType::Prompts => 2,
            ResourceType::Themes => 3,
        }
    };
    for group in &mut groups {
        group
            .subgroups
            .sort_by_key(|sg| type_order(&sg.resource_type));
        for subgroup in &mut group.subgroups {
            subgroup
                .items
                .sort_by(|a, b| a.display_name.cmp(&b.display_name));
        }
    }

    groups
}

/// Build flat entries from groups
pub fn flatten_groups(groups: &[ResourceGroup]) -> Vec<FlatEntry> {
    let mut entries = Vec::new();
    for group in groups {
        entries.push(FlatEntry::Group(group.clone()));
        for subgroup in &group.subgroups {
            entries.push(FlatEntry::Subgroup(subgroup.clone()));
            for item in &subgroup.items {
                entries.push(FlatEntry::Item(item.clone()));
            }
        }
    }
    entries
}

/// Filter entries by search query
pub fn filter_entries(entries: &[FlatEntry], query: &str) -> Vec<FlatEntry> {
    if query.trim().is_empty() {
        return entries.to_vec();
    }

    let lower = query.to_lowercase();

    // Find matching items
    let matching_paths: std::collections::HashSet<String> = entries
        .iter()
        .filter_map(|e| {
            if let FlatEntry::Item(item) = e {
                if item.display_name.to_lowercase().contains(&lower)
                    || item.path.to_lowercase().contains(&lower)
                    || item.resource_type.label().to_lowercase().contains(&lower)
                {
                    Some(item.path.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    // Find which groups/subgroups have matching items
    let mut matching_groups = std::collections::HashSet::new();
    let mut matching_subgroups = std::collections::HashSet::new();

    for entry in entries {
        if let FlatEntry::Item(item) = entry {
            if matching_paths.contains(&item.path) {
                let sub_key = format!("{}:{}", item.resource_type.label(), item.path);
                matching_subgroups.insert(sub_key);

                let group_key = format!(
                    "{}:{}:{}:{}",
                    item.metadata.origin,
                    item.metadata.scope,
                    item.metadata.source,
                    item.metadata.base_dir.as_deref().unwrap_or("")
                );
                matching_groups.insert(group_key);
            }
        }
    }

    let mut result = Vec::new();
    for entry in entries {
        match entry {
            FlatEntry::Group(g) => {
                // Check if any subgroup in this group matched
                let has_matching = g.subgroups.iter().any(|sg| {
                    sg.items
                        .iter()
                        .any(|item| matching_paths.contains(&item.path))
                });
                if has_matching {
                    result.push(entry.clone());
                }
            }
            FlatEntry::Subgroup(sg) => {
                let has_matching = sg
                    .items
                    .iter()
                    .any(|item| matching_paths.contains(&item.path));
                if has_matching {
                    result.push(entry.clone());
                }
            }
            FlatEntry::Item(item) => {
                if matching_paths.contains(&item.path) {
                    result.push(entry.clone());
                }
            }
        }
    }

    result
}

/// Find the first item index in flat entries
pub fn find_first_item(entries: &[FlatEntry]) -> usize {
    entries
        .iter()
        .position(|e| matches!(e, FlatEntry::Item(_)))
        .unwrap_or(0)
}

/// Find the next item index in a direction
pub fn find_next_item(entries: &[FlatEntry], from: usize, direction: i32) -> usize {
    let mut idx = from as i32 + direction;
    while idx >= 0 && (idx as usize) < entries.len() {
        if matches!(&entries[idx as usize], FlatEntry::Item(_)) {
            return idx as usize;
        }
        idx += direction;
    }
    from
}

/// Render config selector header
pub fn render_config_selector_header(width: usize) -> Vec<String> {
    let title = ToolTheme::bold("Resource Configuration");
    let sep = ToolTheme::fg("muted", " · ");
    let hint = format!("space: toggle{}esc: close", sep);
    let spacing = width.saturating_sub(title.len() + hint.len());
    vec![
        format!(
            "{}{}{}",
            title,
            " ".repeat(spacing),
            ToolTheme::fg("accent", &hint)
        ),
        ToolTheme::fg("muted", "Type to filter resources"),
    ]
}

/// Render resource list
pub fn render_resource_list(
    entries: &[FlatEntry],
    selected_index: usize,
    search_query: &str,
    width: usize,
    max_visible: usize,
) -> Vec<String> {
    let mut lines = Vec::new();

    // Search input
    let search_display = if search_query.is_empty() {
        ToolTheme::fg("muted", "  Type to filter...")
    } else {
        format!("  {}", search_query)
    };
    lines.push(search_display);
    lines.push(String::new());

    if entries.is_empty() {
        lines.push(ToolTheme::fg("muted", "  No resources found"));
        return lines;
    }

    let total = entries.len();
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
        if let Some(entry) = entries.get(i) {
            match entry {
                FlatEntry::Group(group) => {
                    lines.push(ToolTheme::fg(
                        "accent",
                        &format!("\x1b[1m  {}\x1b[22m", group.label),
                    ));
                }
                FlatEntry::Subgroup(subgroup) => {
                    lines.push(ToolTheme::fg("muted", &format!("    {}", subgroup.label)));
                }
                FlatEntry::Item(item) => {
                    let is_selected = i == selected_index;
                    let cursor = if is_selected { "> " } else { "  " };
                    let checkbox = if item.enabled {
                        ToolTheme::fg("success", "[x]")
                    } else {
                        ToolTheme::fg("dim", "[ ]")
                    };
                    let name = if is_selected {
                        ToolTheme::bold(&item.display_name)
                    } else {
                        item.display_name.clone()
                    };
                    let line = format!("{}    {} {}", cursor, checkbox, name);
                    let truncated = if line.len() > width {
                        format!("{}...", &line[..width.saturating_sub(3)])
                    } else {
                        line
                    };
                    lines.push(truncated);
                }
            }
        }
    }

    if total > max_visible {
        let item_count = entries
            .iter()
            .filter(|e| matches!(e, FlatEntry::Item(_)))
            .count();
        let current_item = entries[..=selected_index]
            .iter()
            .filter(|e| matches!(e, FlatEntry::Item(_)))
            .count();
        lines.push(ToolTheme::fg(
            "dim",
            &format!("  ({}/{})", current_item, item_count),
        ));
    }

    lines
}
