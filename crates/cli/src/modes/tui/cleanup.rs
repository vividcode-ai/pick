use super::context::TuiContext;

/// Disable bracketed paste mode, restore terminal, print session resume box
pub(crate) fn cleanup_tui_mode(ctx: &mut TuiContext) {
    // Disable bracketed paste mode
    let _ = print!("\x1b[?2004l");
    let _ = std::io::Write::flush(&mut std::io::stdout());

    // Cleanup terminal manager first (restore cursor, clear overflow)
    let _ = ctx.terminal_manager.cleanup();

    // TUI app cleanup (disable raw mode, print newline)
    ctx.tui.cleanup();

    // Print session resume hint in a rounded box
    print_session_box(&ctx.session_manager, ctx.version);
}

/// Print session resume hint box to stderr
fn print_session_box(
    session_manager: &pick_agent::session::SessionManager,
    version: &str,
) {
    let path = session_manager.session_path();
    let header = session_manager.header();
    if let (Some(_path), Some(hdr)) = (path, header) {
        let id = &hdr.id;
        let short_id = if id.len() > 8 {
            &id[..8]
        } else {
            id.as_str()
        };
        let msg_count = session_manager.entries().len();

        let mut box_content: Vec<String> = Vec::new();
        box_content.push(format!("🤖 Pick v{}", version));
        box_content.push(String::new());
        box_content.push("To continue this session, run:".to_string());
        box_content.push("   Pick -c".to_string());
        box_content.push(format!("   Pick -c {}", id));
        if short_id != id {
            box_content.push(format!("   Pick -c {}", short_id));
        }
        box_content.push(String::new());
        box_content.push(format!(
            "Session: {} messages  ID: {}",
            msg_count, short_id
        ));

        let natural = box_content
            .iter()
            .map(|l| pick_tui::utils::visible_width(l))
            .max()
            .unwrap_or(0);
        let max_inner = crossterm::terminal::size()
            .map(|(w, _)| (w as usize).saturating_sub(4))
            .unwrap_or(76)
            .min(120);
        let inner = natural.max(60).min(max_inner);

        let box_line = |content: &str| -> String {
            let content_width = inner - 1;
            let vis = pick_tui::utils::visible_width(content);
            let pad = content_width.saturating_sub(vis);
            format!("│ {}{}│", content, "\u{00a0}".repeat(pad))
        };

        eprintln!();
        eprintln!("╭{}╮", "─".repeat(inner));
        for line in &box_content {
            eprintln!("{}", box_line(line));
        }
        eprintln!("╰{}╯", "─".repeat(inner));
    }
}
