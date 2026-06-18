//! Session manager

pub mod tree;

pub use tree::*;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use super::entries::{LabelEntry, SessionEntry, SessionEntryKind, SessionHeader};
use super::goal::GoalManager;
use super::storage::{JsonlStorage, SessionStorage, StorageError};

/// Session manager handles session lifecycle
pub struct SessionManager {
    storage: Box<dyn SessionStorage>,
    header: Option<SessionHeader>,
    entries: Vec<SessionEntry>,
    session_path: Option<PathBuf>,
    /// Current leaf ID for tree navigation.
    /// Determines which entries form the active context.
    pub leaf_id: Option<String>,
    /// Labels indexed by target entry ID
    labels_by_id: HashMap<String, String>,
    label_timestamps_by_id: HashMap<String, String>,
    /// Thread-level goal state manager
    goal_manager: Arc<GoalManager>,
}

impl SessionManager {
    /// Create a new session in the given directory
    pub async fn create(cwd: PathBuf, session_dir: Option<PathBuf>) -> Result<Self, StorageError> {
        let dir = session_dir.clone().unwrap_or_else(|| cwd.join(".pick").join("sessions"));
        let id = uuid::Uuid::now_v7().to_string();
        let path = dir.join(format!("{}.jsonl", id));
        std::fs::create_dir_all(&dir).ok();
        let storage = Box::new(JsonlStorage::new(path.clone()));

        let header = SessionHeader {
            id,
            version: 1,
            created_at: chrono::Utc::now().timestamp_millis(),
            updated_at: chrono::Utc::now().timestamp_millis(),
            cwd: Some(cwd.to_string_lossy().to_string()),
            model: None,
            provider: None,
        };
        storage.save_header(header.clone()).await?;

        // Add default placeholder title
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ");
        let title_entry = SessionEntry {
            id: uuid::Uuid::now_v7().to_string(),
            parent_id: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
            kind: SessionEntryKind::SessionInfo(super::entries::SessionInfoEntry {
                name: format!("New session - {}", now),
            }),
        };
        storage.append(title_entry.clone()).await?;

        Ok(Self {
            storage,
            header: Some(header),
            entries: vec![title_entry],
            session_path: Some(path),
            leaf_id: None,
            labels_by_id: HashMap::new(),
            label_timestamps_by_id: HashMap::new(),
            goal_manager: Arc::new(GoalManager::new()),
        })
    }

    /// Rebuild label index from all Label entries
    fn rebuild_label_index(&mut self) {
        self.labels_by_id.clear();
        self.label_timestamps_by_id.clear();
        for entry in &self.entries {
            if let SessionEntryKind::Label(le) = &entry.kind {
                if let Some(label) = &le.label {
                    self.labels_by_id.insert(le.target_id.clone(), label.clone());
                    self.label_timestamps_by_id
                        .insert(le.target_id.clone(), entry.timestamp.to_string());
                } else {
                    self.labels_by_id.remove(&le.target_id);
                    self.label_timestamps_by_id.remove(&le.target_id);
                }
            }
        }
    }

    /// Open an existing session from a file
    pub async fn open(path: PathBuf, _cwd: PathBuf) -> Result<Self, StorageError> {
        let storage = JsonlStorage::new(path.clone());
        let header = storage.load_header().await?;
        let entries = storage.load().await?;

        let goal_manager = Arc::new(GoalManager::new());
        Self::restore_goal_from_entries(&goal_manager, &entries);

        let leaf_id = Self::detect_leaf_id_inner(&entries);
        let mut mgr = Self {
            storage: Box::new(storage),
            header,
            entries,
            session_path: Some(path),
            leaf_id,
            labels_by_id: HashMap::new(),
            label_timestamps_by_id: HashMap::new(),
            goal_manager,
        };
        mgr.rebuild_label_index();
        Ok(mgr)

    }

    /// Detect leaf_id from a slice of entries (static helper)
    fn detect_leaf_id_inner(entries: &[SessionEntry]) -> Option<String> {
        let child_ids: std::collections::HashSet<&str> = entries
            .iter()
            .filter_map(|e| e.parent_id.as_deref())
            .collect();
        entries
            .iter()
            .filter(|e| !child_ids.contains(e.id.as_str()))
            .max_by_key(|e| e.timestamp)
            .map(|e| e.id.clone())
    }

    /// Fork a session from an existing one
    pub async fn fork_from(source_path: PathBuf, cwd: PathBuf) -> Result<Self, StorageError> {
        let source_storage = JsonlStorage::new(source_path);
        let header = source_storage.load_header().await?;
        let entries = source_storage.load().await?;

        let mut new_header = header.clone().unwrap_or(SessionHeader {
            id: uuid::Uuid::now_v7().to_string(),
            version: 1,
            created_at: chrono::Utc::now().timestamp_millis(),
            updated_at: chrono::Utc::now().timestamp_millis(),
            cwd: Some(cwd.to_string_lossy().to_string()),
            model: None,
            provider: None,
        });
        new_header.id = uuid::Uuid::now_v7().to_string();
        new_header.cwd = Some(cwd.to_string_lossy().to_string());

        let session_path = cwd.join(".pick").join("sessions").join(&format!("{}.jsonl", new_header.id));
        if let Some(parent) = session_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(StorageError::Io)?;
        }
        let storage = JsonlStorage::new(session_path.clone());
        storage.save_header(new_header.clone()).await?;

        // Update latest session_info entry with fork numbering
        let mut entries = entries;
        if let Some(idx) = entries.iter().rposition(|e| matches!(&e.kind, SessionEntryKind::SessionInfo(_))) {
            if let SessionEntryKind::SessionInfo(ref info) = entries[idx].kind.clone() {
                let forked_name = Self::forked_title(&info.name);
                entries[idx].kind = SessionEntryKind::SessionInfo(super::entries::SessionInfoEntry {
                    name: forked_name,
                });
            }
        }

        let goal_manager = Arc::new(GoalManager::new());
        Self::restore_goal_from_entries(&goal_manager, &entries);

        let leaf_id = Self::detect_leaf_id_inner(&entries);
        let mut mgr = Self {
            storage: Box::new(storage),
            header: Some(new_header),
            entries,
            session_path: Some(session_path),
            leaf_id,
            goal_manager,
            labels_by_id: HashMap::new(),
            label_timestamps_by_id: HashMap::new(),
        };
        mgr.rebuild_label_index();
        Ok(mgr)
    }

    /// Restore goal state from persisted entries (called on session open/fork)
    fn restore_goal_from_entries(goal_manager: &Arc<GoalManager>, entries: &[SessionEntry]) {
        for entry in entries.iter().rev() {
            if let SessionEntryKind::Goal(ref goal) = entry.kind {
                goal_manager.load(goal.clone());
                break;
            }
        }
    }

    /// Get a reference to the goal manager
    pub fn goal_manager(&self) -> Arc<GoalManager> {
        self.goal_manager.clone()
    }

    /// Persist the current goal state to the session
    pub async fn persist_goal(&mut self) -> Result<(), StorageError> {
        if let Some(goal) = self.goal_manager.get() {
            let entry = SessionEntry {
                id: uuid::Uuid::now_v7().to_string(),
                parent_id: self.leaf_id.clone(),
                timestamp: chrono::Utc::now().timestamp_millis(),
                kind: SessionEntryKind::Goal(goal),
            };
            self.append(entry).await?;
        }
        Ok(())
    }

    /// Clear and persist the goal removal
    pub async fn clear_goal(&mut self) -> Result<(), StorageError> {
        self.goal_manager.clear().ok();
        Ok(())
    }

    /// Append an entry to the session
    pub async fn append(&mut self, mut entry: SessionEntry) -> Result<(), StorageError> {
        // If entry has no parent_id, use current leaf_id
        if entry.parent_id.is_none() {
            entry.parent_id = self.leaf_id.clone();
        }
        // If no leaf_id yet, use this entry as the leaf
        if self.leaf_id.is_none() {
            self.leaf_id = Some(entry.id.clone());
        }
        self.storage.append(entry.clone()).await?;
        self.entries.push(entry);
        Ok(())
    }

    /// Get all entries
    pub fn entries(&self) -> &[SessionEntry] {
        &self.entries
    }

    /// Get session header
    pub fn header(&self) -> Option<&SessionHeader> {
        self.header.as_ref()
    }

    /// Get the file path of this session, if known
    pub fn session_path(&self) -> Option<&std::path::Path> {
        self.session_path.as_deref()
    }

    /// Get the path from an entry to the root (oldest ancestor).
    /// Returns entries in order from root to the given entry.
    pub fn get_path_to_root(&self, entry_id: &str) -> Vec<&SessionEntry> {
        let mut path: Vec<&SessionEntry> = Vec::new();

        // Build a map from entry id to entry
        let entry_map: HashMap<&str, &SessionEntry> =
            self.entries.iter().map(|e| (e.id.as_str(), e)).collect();

        // Walk up the parent chain
        let mut current = entry_map.get(entry_id);
        while let Some(entry) = current {
            path.push(*entry);
            current = entry.parent_id.as_deref().and_then(|pid| entry_map.get(pid));
        }

        path.reverse();
        path
    }

    /// Find all entries of a specific kind
    pub fn find_entries_by_kind(&self, kind: &str) -> Vec<&SessionEntry> {
        self.entries.iter().filter(|e| {
            let entry_kind = match &e.kind {
                super::entries::SessionEntryKind::Message(_) => "message",
                super::entries::SessionEntryKind::Compaction(_) => "compaction",
                super::entries::SessionEntryKind::BranchSummary(_) => "branch_summary",
                super::entries::SessionEntryKind::ModelChange(_) => "model_change",
                super::entries::SessionEntryKind::ThinkingLevelChange(_) => "thinking_level_change",
                super::entries::SessionEntryKind::Custom(_) => "custom",
                super::entries::SessionEntryKind::SessionInfo(_) => "session_info",
                super::entries::SessionEntryKind::LeafChange(_) => "leaf_change",
                super::entries::SessionEntryKind::Label(_) => "label",
                super::entries::SessionEntryKind::AgentModeChange(_) => "agent_mode_change",
                super::entries::SessionEntryKind::TodoUpdate(_) => "todo_update",
                super::entries::SessionEntryKind::Goal(_) => "goal",
            };
            entry_kind == kind
        }).collect()
    }

    /// Get leaf entries (entries with no children) - these represent
    /// the latest state on each conversation branch.
    pub fn get_leaf_entries(&self) -> Vec<&SessionEntry> {
        let child_ids: HashSet<&str> = self.entries.iter()
            .filter_map(|e| e.parent_id.as_deref())
            .collect();

        self.entries.iter()
            .filter(|e| !child_ids.contains(e.id.as_str()))
            .collect()
    }

    /// Get entries that have a specific parent (direct children).
    /// Returns entries in chronological order.
    pub fn get_children(&self, parent_id: &str) -> Vec<&SessionEntry> {
        let mut children: Vec<&SessionEntry> = self.entries.iter()
            .filter(|e| e.parent_id.as_deref() == Some(parent_id))
            .collect();
        children.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        children
    }

    /// Build a context from entries by walking the path to root from the given leaf entry.
    /// Returns entries in chronological order from root to the leaf.
    pub fn build_context_from_leaf(&self, leaf_id: &str) -> Vec<&SessionEntry> {
        self.get_path_to_root(leaf_id)
    }

    // ── Session name ─────────────────────────────────────────────────

    /// Append a session_info entry to set the session name
    pub async fn append_session_info(&mut self, name: &str) -> Result<(), StorageError> {
        let entry = SessionEntry {
            id: uuid::Uuid::now_v7().to_string(),
            parent_id: self.leaf_id.clone(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            kind: SessionEntryKind::SessionInfo(super::entries::SessionInfoEntry {
                name: name.to_string(),
            }),
        };
        self.append(entry).await
    }

    /// Get the latest session name by scanning entries in reverse
    pub fn get_session_name(&self) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find_map(|e| {
                if let SessionEntryKind::SessionInfo(info) = &e.kind {
                    Some(info.name.as_str())
                } else {
                    None
                }
            })
    }

    // ── Leaf / Branch navigation ─────────────────────────────────────

    /// Set the current leaf ID (used when navigating the session tree)
    pub fn set_leaf_id(&mut self, leaf_id: &str) {
        self.leaf_id = Some(leaf_id.to_string());
    }

    /// Get the current leaf ID
    pub fn get_leaf_id(&self) -> Option<&str> {
        self.leaf_id.as_deref()
    }

    /// Record a leaf change entry to the session JSONL
    pub async fn append_leaf_change(
        &mut self,
        from: Option<String>,
        to: &str,
    ) -> Result<(), StorageError> {
        let entry = SessionEntry {
            id: uuid::Uuid::now_v7().to_string(),
            parent_id: Some(to.to_string()),
            timestamp: chrono::Utc::now().timestamp_millis(),
            kind: SessionEntryKind::LeafChange(super::entries::LeafChangeEntry {
                from,
                to: to.to_string(),
            }),
        };
        self.append(entry).await
    }

    // ── Label management ──────────────────────────────────────────────

    /// Append a label entry for the given target entry
    pub async fn append_label_change(
        &mut self,
        target_id: &str,
        label: Option<&str>,
    ) -> Result<(), StorageError> {
        let entry = SessionEntry {
            id: uuid::Uuid::now_v7().to_string(),
            parent_id: self.leaf_id.clone(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            kind: SessionEntryKind::Label(LabelEntry {
                target_id: target_id.to_string(),
                label: label.map(|s| s.to_string()),
            }),
        };
        if let Some(l) = label {
            self.labels_by_id.insert(target_id.to_string(), l.to_string());
            self.label_timestamps_by_id.insert(target_id.to_string(), entry.timestamp.to_string());
        } else {
            self.labels_by_id.remove(target_id);
            self.label_timestamps_by_id.remove(target_id);
        }
        self.append(entry).await
    }

    /// Get the label for a given entry ID, if any
    pub fn get_label(&self, entry_id: &str) -> Option<&str> {
        self.labels_by_id.get(entry_id).map(|s| s.as_str())
    }

    // ── Tree building ─────────────────────────────────────────────────

    /// Compute the set of entry IDs on the active path (from leaf to root)
    pub fn compute_active_path(&self) -> HashSet<&str> {
        let mut path = HashSet::new();
        if let Some(ref leaf) = self.leaf_id {
            let entry_map: HashMap<&str, &SessionEntry> =
                self.entries.iter().map(|e| (e.id.as_str(), e)).collect();
            let mut current: Option<&str> = Some(leaf.as_str());
            while let Some(id) = current {
                path.insert(id);
                current = entry_map.get(id).and_then(|e| e.parent_id.as_deref());
            }
        }
        path
    }

    /// Build tree data for TUI rendering.
    pub fn build_tree(&self) -> Vec<tree::TreeNodeData> {
        if self.entries.is_empty() { return Vec::new(); }
        let mut children_map: HashMap<&str, Vec<&SessionEntry>> = HashMap::new();
        for entry in &self.entries {
            let pid = entry.parent_id.as_deref().unwrap_or("");
            children_map.entry(pid).or_default().push(entry);
        }
        for (_, children) in children_map.iter_mut() {
            children.sort_by_key(|e| e.timestamp);
        }
        let roots: Vec<&SessionEntry> = self.entries.iter()
            .filter(|e| e.parent_id.is_none() || e.parent_id.as_deref() == Some(""))
            .collect();
        if roots.is_empty() { return Vec::new(); }

        let mut result: Vec<tree::TreeNodeData> = Vec::new();
        let mut stack: Vec<(usize, &SessionEntry, bool, Vec<bool>)> = Vec::new();
        for (i, root) in roots.iter().enumerate() {
            let is_last = i == roots.len() - 1 && stack.is_empty();
            stack.push((0, root, is_last, Vec::new()));
        }
        while let Some((depth, entry, is_last, gutters)) = stack.pop() {
            let children = children_map.get(entry.id.as_str());
            let child_count = children.map(|c| c.len()).unwrap_or(0);
            let label = self.labels_by_id.get(&entry.id).cloned();
            let label_timestamp = self.label_timestamps_by_id.get(&entry.id).cloned();
            result.push(tree::TreeNodeData {
                entry_id: entry.id.clone(), parent_id: entry.parent_id.clone(),
                depth, has_children: child_count > 0, is_last,
                gutters: gutters.clone(), label, label_timestamp, entry: entry.clone(),
            });
            if let Some(children) = children {
                for (i, child) in children.iter().enumerate().rev() {
                    let child_is_last = i == child_count - 1;
                    let mut child_gutters = gutters.clone();
                    // Only push gutter when parent has multiple children (branch point)
                    let parent_is_branch = child_count > 1;
                    child_gutters.push(parent_is_branch && i < child_count - 1);
                    stack.push((depth + 1, child, child_is_last, child_gutters));
                }
            }
        }
        result
    }

    // ── Branch summarization helpers ─────────────────────────────────

    pub fn find_common_ancestor(&self, id_a: &str, id_b: &str) -> Option<String> {
        let entry_map: HashMap<&str, &SessionEntry> =
            self.entries.iter().map(|e| (e.id.as_str(), e)).collect();
        let mut path_a: Vec<&str> = Vec::new();
        let mut current = Some(id_a);
        while let Some(id) = current {
            path_a.push(id);
            current = entry_map.get(id).and_then(|e| e.parent_id.as_deref());
        }
        let mut current = Some(id_b);
        while let Some(id) = current {
            if path_a.contains(&id) { return Some(id.to_string()); }
            current = entry_map.get(id).and_then(|e| e.parent_id.as_deref());
        }
        None
    }

    pub fn collect_entries_for_summary<'a>(&'a self, old_leaf: &str, target_id: &str) -> Vec<&'a SessionEntry> {
        let ancestor = match self.find_common_ancestor(old_leaf, target_id) {
            Some(a) => a, None => return Vec::new(),
        };
        let entry_map: HashMap<&str, &SessionEntry> =
            self.entries.iter().map(|e| (e.id.as_str(), e)).collect();
        let mut result: Vec<&SessionEntry> = Vec::new();
        let mut current = entry_map.get(old_leaf);
        while let Some(entry) = current {
            if entry.id == ancestor { break; }
            result.push(*entry);
            current = entry.parent_id.as_deref().and_then(|pid| entry_map.get(pid));
        }
        result.reverse();
        result
    }

    /// Build tree display data: Vec of (depth, entry, has_children, is_last)
    /// for rendering with indentation and tree connectors.
    pub fn build_tree_display(&self) -> Vec<(usize, &SessionEntry, bool, bool)> {
        if self.entries.is_empty() {
            return Vec::new();
        }

        // Build a map from parent_id to children
        let mut children_map: HashMap<&str, Vec<&SessionEntry>> = HashMap::new();
        for entry in &self.entries {
            let pid = entry.parent_id.as_deref().unwrap_or("");
            children_map.entry(pid).or_default().push(entry);
        }

        // Sort children by timestamp
        for (_, children) in children_map.iter_mut() {
            children.sort_by_key(|e| e.timestamp);
        }

        // Find roots: entries with no parent (parent_id is None or empty)
        let roots: Vec<&SessionEntry> = self
            .entries
            .iter()
            .filter(|e| e.parent_id.is_none() || e.parent_id.as_deref() == Some(""))
            .collect();

        if roots.is_empty() {
            return Vec::new();
        }

        let mut result: Vec<(usize, &SessionEntry, bool, bool)> = Vec::new();
        // Build the tree by traversing from each root
        let mut stack: Vec<(usize, &SessionEntry, bool)> = Vec::new();
        for (i, root) in roots.iter().enumerate() {
            let is_last = i == roots.len() - 1 && stack.is_empty();
            stack.push((0, root, is_last));
        }

        while let Some((depth, entry, is_last)) = stack.pop() {
            let children = children_map.get(entry.id.as_str());
            let has_children = children.map(|c| !c.is_empty()).unwrap_or(false);
            result.push((depth, entry, has_children, is_last));

            if let Some(children) = children {
                for (i, child) in children.iter().enumerate().rev() {
                    let child_is_last = i == children.len() - 1;
                    stack.push((depth + 1, child, child_is_last));
                }
            }
        }

        result
    }

    /// Generate a forked title by appending `(fork #N)` suffix
    fn forked_title(title: &str) -> String {
        if let Some(pos) = title.rfind(" (fork #") {
            if let Some(end) = title[pos + 8..].find(')') {
                if let Ok(num) = title[pos + 8..pos + 8 + end].parse::<u32>() {
                    return format!("{} (fork #{})", &title[..pos], num + 1);
                }
            }
        }
        format!("{} (fork #1)", title)
    }
}
