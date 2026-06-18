//! TUI config selector for `Pick config` command

use crate::utils::tui_wrapper::{SelectResult, run_list_selector};

/// Options for the config selector
pub struct ConfigSelectorOptions {
    pub cwd: String,
    pub agent_dir: String,
}

/// Show TUI config selector and return when closed
pub async fn select_config(options: ConfigSelectorOptions) {
    let items = vec![
        format!("Extensions ({}/extensions)", options.agent_dir),
        format!("Skills ({}/skills)", options.agent_dir),
        format!("Prompts ({}/prompts)", options.agent_dir),
        format!("Themes ({}/themes)", options.agent_dir),
        format!("Settings ({}/settings.json)", options.agent_dir),
    ];

    match run_list_selector("Resource Configuration", &items, |s| s.clone()) {
        SelectResult::Selected(_idx) => {
            // In a full implementation, this would show a detailed editor view.
            // For now, selection closes the selector.
        }
        SelectResult::Cancelled => {}
    }
}
