//! Session storage backends

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::entries::{SessionEntry, SessionHeader};

/// Storage backend for sessions
#[async_trait::async_trait]
pub trait SessionStorage: Send + Sync {
    /// Load session entries
    async fn load(&self) -> Result<Vec<SessionEntry>, StorageError>;

    /// Append a new entry
    async fn append(&self, entry: SessionEntry) -> Result<(), StorageError>;

    /// Load session header
    async fn load_header(&self) -> Result<Option<SessionHeader>, StorageError>;

    /// Save session header
    async fn save_header(&self, header: SessionHeader) -> Result<(), StorageError>;
}

/// Storage errors
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Not found")]
    NotFound,
}

/// JSONL file-based session storage
pub struct JsonlStorage {
    path: PathBuf,
}

impl JsonlStorage {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait::async_trait]
impl SessionStorage for JsonlStorage {
    async fn load(&self) -> Result<Vec<SessionEntry>, StorageError> {
        if !self.path.exists() {
            return Ok(vec![]);
        }
        let content = tokio::fs::read_to_string(&self.path).await?;
        let entries: Vec<SessionEntry> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        Ok(entries)
    }

    async fn append(&self, entry: SessionEntry) -> Result<(), StorageError> {
        let line = serde_json::to_string(&entry)?;
        let mut content = if self.path.exists() {
            tokio::fs::read_to_string(&self.path).await?
        } else {
            String::new()
        };
        content.push_str(&line);
        content.push('\n');
        tokio::fs::write(&self.path, content).await?;
        Ok(())
    }

    async fn load_header(&self) -> Result<Option<SessionHeader>, StorageError> {
        if !self.path.exists() {
            return Ok(None);
        }
        let content = tokio::fs::read_to_string(&self.path).await?;
        if let Some(first_line) = content.lines().next()
            && let Ok(header) = serde_json::from_str::<SessionHeader>(first_line)
        {
            return Ok(Some(header));
        }
        Ok(None)
    }

    async fn save_header(&self, header: SessionHeader) -> Result<(), StorageError> {
        let existing = if self.path.exists() {
            let content = tokio::fs::read_to_string(&self.path).await?;
            let lines: Vec<&str> = content.lines().collect();
            if lines.len() > 1 {
                lines[1..].join("\n")
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let header_line = serde_json::to_string(&header)?;
        let content = if existing.is_empty() {
            header_line
        } else {
            format!("{}\n{}", header_line, existing)
        };
        tokio::fs::write(&self.path, content + "\n").await?;
        Ok(())
    }
}

impl JsonlStorage {
    /// Compact the JSONL file by removing non-essential entries
    /// while preserving the conversation tree structure.
    ///
    /// Keeps: Message, SessionInfo, LeafChange, Goal entries.
    /// Removes: Compaction, BranchSummary, Label, ModelChange, etc.
    ///
    /// For entries whose parent_id points to a removed entry, the parent_id
    /// is re-wired to the nearest kept ancestor (or None if none exists).
    pub async fn compact(&self) -> Result<(), StorageError> {
        let entries = self.load().await?;
        let header = self.load_header().await?;

        use super::entries::SessionEntryKind;
        fn is_essential(kind: &SessionEntryKind) -> bool {
            matches!(
                kind,
                SessionEntryKind::Message(_)
                    | SessionEntryKind::SessionInfo(_)
                    | SessionEntryKind::LeafChange(_)
                    | SessionEntryKind::Goal(_)
            )
        }

        let mut kept: Vec<super::entries::SessionEntry> = entries
            .iter()
            .filter(|e| is_essential(&e.kind))
            .cloned()
            .collect();

        let kept_ids: std::collections::HashSet<String> =
            kept.iter().map(|e| e.id.clone()).collect();

        let parent_map: std::collections::HashMap<String, Option<String>> = entries
            .iter()
            .map(|e| (e.id.clone(), e.parent_id.clone()))
            .collect();

        fn find_kept_ancestor(
            id: &str,
            kept_ids: &std::collections::HashSet<String>,
            parent_map: &std::collections::HashMap<String, Option<String>>,
        ) -> Option<String> {
            let mut current = parent_map.get(id)?.as_ref()?.clone();
            loop {
                if kept_ids.contains(&current) {
                    return Some(current);
                }
                current = parent_map.get(&current)?.as_ref()?.clone();
            }
        }

        for entry in &mut kept {
            if let Some(ref pid) = entry.parent_id
                && !kept_ids.contains(pid)
            {
                entry.parent_id = find_kept_ancestor(pid, &kept_ids, &parent_map);
            }
        }

        let mut content = String::new();
        if let Some(ref h) = header
            && let Ok(line) = serde_json::to_string(h)
        {
            content.push_str(&line);
            content.push('\n');
        }
        for entry in &kept {
            if let Ok(line) = serde_json::to_string(entry) {
                content.push_str(&line);
                content.push('\n');
            }
        }
        tokio::fs::write(&self.path, content).await?;
        Ok(())
    }
}

/// In-memory session storage (for testing)
pub struct MemoryStorage {
    entries: Arc<Mutex<Vec<SessionEntry>>>,
    header: Arc<Mutex<Option<SessionHeader>>>,
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
            header: Arc::new(Mutex::new(None)),
        }
    }
}

#[async_trait::async_trait]
impl SessionStorage for MemoryStorage {
    async fn load(&self) -> Result<Vec<SessionEntry>, StorageError> {
        Ok(self.entries.lock().unwrap().clone())
    }

    async fn append(&self, entry: SessionEntry) -> Result<(), StorageError> {
        self.entries.lock().unwrap().push(entry);
        Ok(())
    }

    async fn load_header(&self) -> Result<Option<SessionHeader>, StorageError> {
        Ok(self.header.lock().unwrap().clone())
    }

    async fn save_header(&self, header: SessionHeader) -> Result<(), StorageError> {
        *self.header.lock().unwrap() = Some(header);
        Ok(())
    }
}
