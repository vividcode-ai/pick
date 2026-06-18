//! Footer status bar component


use crate::core::tools::render_utils::ToolTheme;

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

    // Try to make path relative to home
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

/// Render a complete footer status bar
pub fn render_footer(
    _width: usize,
    cwd: &str,
    home: Option<&str>,
    total_input: u64,
    total_output: u64,
    total_cache_read: u64,
    total_cache_write: u64,
    context_percent: Option<f64>,
    context_window: u64,
    model_name: &str,
    provider_count: usize,
    thinking_level: &str,
    git_branch: Option<&str>,
    session_name: Option<&str>,
    auto_compact: bool,
    _using_subscription: bool,
) -> Vec<String> {
    // Build stats parts
    let mut stats_parts: Vec<String> = Vec::new();
    if total_input > 0 {
        stats_parts.push(format!("↑{}", format_tokens(total_input)));
    }
    if total_output > 0 {
        stats_parts.push(format!("↓{}", format_tokens(total_output)));
    }
    if total_cache_read > 0 {
        stats_parts.push(format!("R{}", format_tokens(total_cache_read)));
    }
    if total_cache_write > 0 {
        stats_parts.push(format!("W{}", format_tokens(total_cache_write)));
    }

    // Context percentage
    let auto_indicator = if auto_compact { " (auto)" } else { "" };
    let context_display = match context_percent {
        Some(pct) => {
            let display = format!("{:.1}%/{}{}", pct, format_tokens(context_window), auto_indicator);
            if pct > 90.0 {
                ToolTheme::fg("error", &display)
            } else if pct > 70.0 {
                ToolTheme::fg("warning", &display)
            } else {
                display
            }
        }
        None => format!("?/{}{}", format_tokens(context_window), auto_indicator),
    };
    stats_parts.push(context_display);

    let stats_left = stats_parts.join(" ");

    // Build right side (model name)
    let mut right_side = model_name.to_string();
    let has_thinking = thinking_level != "off";
    if has_thinking {
        right_side = format!("{} • {}", model_name, thinking_level);
    }
    if provider_count > 1 {
        right_side = format!("({}) {}", "provider", right_side);
    }

    // PWD line
    let mut pwd_display = format_cwd_for_footer(cwd, home);
    if let Some(branch) = git_branch {
        pwd_display = format!("{} ({})", pwd_display, branch);
    }
    if let Some(name) = session_name {
        pwd_display = format!("{} • {}", pwd_display, name);
    }

    let pwd_line = format!("{}", ToolTheme::fg("dim", &pwd_display));

    // Stats line with left/right alignment
    let stats_line = format!("{}{}", ToolTheme::fg("dim", &stats_left), ToolTheme::fg("dim", &right_side));

    vec![pwd_line, stats_line]
}
