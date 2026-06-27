use std::collections::HashMap;

use pick_agent::permission::approval::PermissionConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompactionSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserve_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_recent_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BranchSummarySettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserve_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_prompt: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarkdownSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_block_indent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderRetrySettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retry_delay_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetrySettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_delay_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderRetrySettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TerminalSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_images: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_width_cells: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clear_on_shrink: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_terminal_progress: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_resize: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_images: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WarningsSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic_extra_usage: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThinkingBudgetsSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimal: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub low: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub medium: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub high: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpServerConfigJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Settings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_changelog_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_thinking_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compaction: Option<CompactionSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_summary: Option<BranchSummarySettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetrySettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_command_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npm_command: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quiet_startup: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_thinking_block: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapse_changelog: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_skill_commands: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steering_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub follow_up_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub double_escape_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree_filter_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_hardware_cursor: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor_padding_x: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autocomplete_max_visible: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_idle_timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packages: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub themes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<MarkdownSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<WarningsSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<TerminalSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<ImageSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budgets: Option<ThinkingBudgetsSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled_models: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_install_telemetry: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<HashMap<String, McpServerConfigJson>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_mcp_servers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission: Option<PermissionConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_mcp_tools: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_for_update_on_startup: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dismissed_update_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_system_notifications: Option<bool>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}
