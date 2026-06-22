//! Data types for the TUI application

use crate::components::chat::ChatView;
use crate::components::select::SelectList;
use crate::editor::Editor;
use crate::paste_burst::PasteBurst;

/// Action returned by the TUI when the user submits input or quits
#[derive(Debug)]
pub enum TuiAction {
    Submit(String),
    SelectionResult(usize, String),
    CycleModel,
    CycleModelBackward,
    SelectModel,
    CycleThinking,
    CycleMode,
    ApiKeySubmit(String),
    Interrupt,
    OpenTree,
    Quit,
    /// Update prompt actions: true = update now, false = skip/dismiss
    UpdateResponse(bool),
}

/// App state
#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Input,
    Selecting,
    Streaming,
    ApiKeyInput,
    TreeSelecting,
    Questioning,
    UpdatePrompt,
}

/// Filter mode for tree display
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

/// A single item in the tree view
#[derive(Debug, Clone)]
pub struct TreeViewItem {
    pub entry_id: String,
    pub parent_id: Option<String>,
    pub depth: usize,
    pub has_children: bool,
    pub is_last: bool,
    pub gutters: Vec<bool>,
    pub label: Option<String>,
    pub label_timestamp: Option<String>,
    pub kind_str: String,
    pub searchable_text: String,
    pub display_label: String,
}

/// Tree view state for interactive session tree navigation
pub struct TreeView {
    pub items: Vec<TreeViewItem>,
    pub(crate) visible_indices: Vec<usize>,
    pub(crate) selected_index: usize,
    pub current_leaf_id: Option<String>,
    pub(crate) active_path_ids: Vec<String>,
    pub(crate) folded_ids: std::collections::HashSet<String>,
    pub filter_mode: TreeFilterMode,
    pub(crate) search_query: String,
    pub(crate) show_label_timestamps: bool,
    pub edit_label_entry_id: Option<String>,
    pub edit_label_buffer: String,
}

/// The main TUI application with startup display and footer
pub struct TuiApp {
    pub chat_lines_written: usize,
    pub chat: ChatView,
    pub editor: Editor,
    pub state: AppState,
    pub provider: String,
    pub model_id: String,
    pub app_name: String,
    pub version: String,
    pub(crate) startup_header_added: bool,
    pub(crate) context_file_names: Vec<String>,
    pub(crate) skill_names: Vec<String>,
    pub cwd: String,
    pub home_dir: Option<String>,
    pub total_input: u64,
    pub total_output: u64,
    pub total_cache_read: u64,
    pub total_cache_write: u64,
    pub context_percent: Option<f64>,
    pub context_window: u64,
    pub git_branch: Option<String>,
    pub session_name: Option<String>,
    pub thinking_level: String,
    pub auto_compact: bool,
    pub agent_mode: String,
    pub selection: Option<SelectList>,
    pub tree_view: Option<TreeView>,
    pub api_key_provider: Option<String>,
    pub api_key_input: String,
    pub(crate) cached_lines_entry_count: usize,
    pub(crate) cached_lines_committed: usize,
    pub(crate) last_render_width: u16,
    pub(crate) autocomplete_space_lines: u16,
    pub status_text: Option<String>,
    pub status_frame: usize,
    pub agent_start_time: Option<std::time::Instant>,
    pub(crate) folder: String,
    pub(crate) last_escape_time: Option<std::time::Instant>,
    pub paste_burst: PasteBurst,
    pub paste_accumulator: String,
    pub last_paste_time: Option<std::time::Instant>,
    pub(crate) last_render_state: AppState,
    pub pending_user_messages: std::collections::VecDeque<String>,
    pub pending_from_flush: bool,
    pub pending_submitted_count: usize,
    pub question_dialog: Option<crate::components::QuestionDialog>,
    pub question_response_tx:
        Option<tokio::sync::oneshot::Sender<Result<Vec<Vec<String>>, String>>>,
    pub update_prompt: Option<crate::components::UpdatePromptState>,
    pub todo_items: Vec<serde_json::Value>,
    pub todo_scroll_offset: usize,
    pub confirm_quit: bool,
    /// Show the hardware terminal cursor (for IME support). When false,
    /// the physical cursor is hidden and only the ratatui virtual cursor is active.
    pub show_hardware_cursor: bool,
}

/// Format token counts for compact display (e.g. 1500 → "1.5k", 15000 → "15k")
pub fn format_tokens(count: u64) -> String {
    if count < 1000 {
        return count.to_string();
    }
    if count < 10000 {
        return format!("{:.1}k", count as f64 / 1000.0);
    }
    if count < 1_000_000 {
        return format!("{}k", count / 1000);
    }
    format!("{:.1}M", count as f64 / 1_000_000.0)
}

/// Format cwd for footer display (shortens home directory to ~)
pub fn format_cwd_for_footer(cwd: &str, home: Option<&str>) -> String {
    let home = match home {
        Some(h) => h,
        None => return cwd.to_string(),
    };

    if let Ok(relative) = std::path::Path::new(cwd).strip_prefix(home) {
        if relative.as_os_str().is_empty() {
            "~".to_string()
        } else {
            format!("~/{}", relative.display())
        }
    } else {
        cwd.to_string()
    }
}

/// Parse a string with ANSI escape sequences into a ratatui styled Line.
/// Uses `ansi-to-tui` crate instead of hand-rolled ANSI parsing.
#[allow(dead_code)]
pub fn ansi_to_styled_line(s: &str) -> ratatui::text::Line<'static> {
    use ansi_to_tui::IntoText as _;
    match s.as_bytes().into_text() {
        Ok(text) => text
            .lines
            .into_iter()
            .next()
            .unwrap_or(ratatui::text::Line::from("")),
        Err(_) => ratatui::text::Line::from(s.to_string()),
    }
}

#[cfg(windows)]
pub(crate) fn set_windows_terminal_title(title: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let wide: Vec<u16> = OsStr::new(title)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        unsafe extern "system" {
            fn SetConsoleTitleW(lpConsoleTitle: *const u16) -> i32;
        }
        SetConsoleTitleW(wide.as_ptr());
    }
}
