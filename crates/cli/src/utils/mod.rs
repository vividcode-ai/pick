//! Shared utility functions

pub mod ansi;
pub mod changelog;
pub mod child_process;
pub mod clipboard;
pub mod frontmatter;
pub mod fs_watch;
pub mod git;
pub mod html;
pub mod image;
pub mod paths;
pub mod shell;
pub mod syntax_highlight;
pub mod tools_manager;
pub mod tui_wrapper;
pub mod user_agent;
pub mod version_check;
pub mod windows_self_update;

// ============================================================================
// MIME Utilities
// ============================================================================

/// Detect supported image MIME type from file extension
pub fn detect_supported_image_mime_type_from_file(path: &str) -> Option<&'static str> {
    image::detect_supported_image_mime_type(path)
}

// ============================================================================
// Sleep
// ============================================================================

/// Async sleep for a given number of milliseconds
pub async fn sleep_ms(ms: u64) {
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
}

// ============================================================================
// User Agent
// ============================================================================

// ============================================================================
// Version Check
// ============================================================================

// ============================================================================
// Changelog
// ============================================================================
