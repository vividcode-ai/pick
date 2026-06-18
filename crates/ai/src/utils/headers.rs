//! HTTP headers utilities

use std::collections::HashMap;

/// HTTP headers collection
#[derive(Debug, Clone)]
pub struct Headers {
    headers: HashMap<String, String>,
}

impl Headers {
    pub fn new() -> Self {
        Self {
            headers: HashMap::new(),
        }
    }

    /// Create headers from a vec of (name, value) pairs
    pub fn from_pairs(pairs: Vec<(String, String)>) -> Self {
        let mut headers = HashMap::new();
        for (k, v) in pairs {
            headers.insert(k.to_lowercase(), v);
        }
        Self { headers }
    }

    /// Set a header value (case-insensitive key)
    pub fn set(&mut self, name: &str, value: &str) {
        self.headers.insert(name.to_lowercase(), value.to_string());
    }

    /// Get a header value (case-insensitive key)
    pub fn get(&self, name: &str) -> Option<&str> {
        self.headers.get(&name.to_lowercase()).map(|s| s.as_str())
    }

    /// Remove a header
    pub fn remove(&mut self, name: &str) {
        self.headers.remove(&name.to_lowercase());
    }

    /// Check if a header exists
    pub fn has(&self, name: &str) -> bool {
        self.headers.contains_key(&name.to_lowercase())
    }

    /// Get all headers as (name, value) pairs
    pub fn all(&self) -> Vec<(&str, &str)> {
        self.headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
    }

    /// Number of headers
    pub fn len(&self) -> usize {
        self.headers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }
}

impl Default for Headers {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Vec<(String, String)>> for Headers {
    fn from(pairs: Vec<(String, String)>) -> Self {
        Self::from_pairs(pairs)
    }
}
