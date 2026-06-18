//! Windows self-update quarantine utilities
//!
//! In the Node.js version, this quarantines native addon files during self-updates
//! to work around Windows file-locking behavior. In Rust, native library locking
//! is handled by the OS, so this module provides the API as a no-op.

use std::path::Path;

const _QUARANTINE_DIR_NAME: &str = ".pick-native-quarantine";

/// Clean up any quarantine directories from previous self-updates
pub fn cleanup_windows_self_update_quarantine(_package_dir: &Path) {
    // No-op in Rust context — native module locking is handled by the OS
}

/// Quarantine native dependencies during self-update
/// In the Node.js version, this renames loaded .node files so they can be
/// replaced. Rust binaries don't have this issue.
pub fn quarantine_windows_native_dependencies(_package_dir: &Path) {
    // No-op in Rust context
}
