//! Hashing utilities

use sha2::{Digest, Sha256};

/// Compute a SHA-256 hex digest of a string
pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}
