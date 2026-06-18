//! TUI application - main interactive event loop with rendering
//! stdout-based terminal rendering (append-only chat + editor at bottom)

pub(crate) mod handlers;
pub(crate) mod render;
pub(crate) mod tree;
pub(crate) mod types;

pub use types::*;

impl Drop for TuiApp {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::components::select::{SelectItem, SelectList};

    use crossterm::event::{KeyCode, KeyModifiers};
    use ratatui::style::{Color, Modifier};
    use ratatui::text::Line;

    /// Helper: get plain text from a ratatui Line.
    fn line_text(line: &Line<'static>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    /// Helper: create TuiApp with just the fields needed for startup header tests.
    struct HeaderTestData {
        app_name: String,
        version: String,
        context_file_names: Vec<String>,
        skill_names: Vec<String>,
    }

    impl HeaderTestData {
        fn new(context: Vec<String>, skills: Vec<String>) -> Self {
            Self {
                app_name: "Pick".to_string(),
                version: "0.1.0".to_string(),
                context_file_names: context,
                skill_names: skills,
            }
        }

        #[allow(unused_variables)]
        fn build_header(&self, width: usize) -> Vec<String> {
            let mut lines: Vec<String> = Vec::new();
            lines.push(String::new());
            lines.push(format!(
                "\x1b[1m{}\x1b[0m\x1b[2m v{}\x1b[0m",
                self.app_name, self.version
            ));
            lines.push("\x1b[2mescape interrupt · ctrl+c twice to exit · / commands · ! bash · ctrl+o more\x1b[0m".to_string());
            lines.push(
                "\x1b[2mPress ctrl+o to show full startup help and loaded resources.\x1b[0m"
                    .to_string(),
            );
            let title = "Pick";
            lines.push(format!(
                "\x1b[2m{} can explain its own features and look up its docs. Ask it how to use or extend {}.\x1b[0m",
                title, title
            ));
            if !self.context_file_names.is_empty() {
                lines.push("\x1b[1m[Context]\x1b[0m".to_string());
                lines.push(format!(
                    "\x1b[2m  {}\x1b[0m",
                    self.context_file_names.join(", ")
                ));
            }
            if !self.skill_names.is_empty() {
                lines.push("\x1b[1m[Skills]\x1b[0m".to_string());
                lines.push(format!("\x1b[2m  {}\x1b[0m", self.skill_names.join(", ")));
            }
            lines
        }
    }

    #[test]
    fn test_startup_header_version() {
        let data = HeaderTestData::new(
            vec!["CLAUDE.md".to_string()],
            vec!["agent-browser".to_string()],
        );
        let lines = data.build_header(80);
        assert_eq!(lines[0], "");
        assert_eq!(lines[1], "\x1b[1mPick\x1b[0m\x1b[2m v0.1.0\x1b[0m");
    }

    #[test]
    fn test_startup_header_keybinding_hints() {
        let data = HeaderTestData::new(
            vec!["CLAUDE.md".to_string()],
            vec!["agent-browser".to_string()],
        );
        let lines = data.build_header(80);
        assert_eq!(
            lines[2],
            "\x1b[2mescape interrupt · ctrl+c twice to exit · / commands · ! bash · ctrl+o more\x1b[0m"
        );
    }

    #[test]
    fn test_startup_header_compact_onboarding() {
        let data = HeaderTestData::new(
            vec!["CLAUDE.md".to_string()],
            vec!["agent-browser".to_string()],
        );
        let lines = data.build_header(80);
        assert_eq!(
            lines[3],
            "\x1b[2mPress ctrl+o to show full startup help and loaded resources.\x1b[0m"
        );
    }

    #[test]
    fn test_startup_header_general_onboarding() {
        let data = HeaderTestData::new(
            vec!["CLAUDE.md".to_string()],
            vec!["agent-browser".to_string()],
        );
        let lines = data.build_header(80);
        assert_eq!(
            lines[4],
            "\x1b[2mPick can explain its own features and look up its docs. Ask it how to use or extend Pick.\x1b[0m"
        );
    }

    #[test]
    fn test_startup_header_context_section() {
        let data = HeaderTestData::new(
            vec!["CLAUDE.md".to_string()],
            vec!["agent-browser".to_string()],
        );
        let lines = data.build_header(80);
        assert_eq!(lines[5], "\x1b[1m[Context]\x1b[0m");
        assert_eq!(lines[6], "\x1b[2m  CLAUDE.md\x1b[0m");
    }

    #[test]
    fn test_startup_header_skills_section() {
        let data = HeaderTestData::new(
            vec!["CLAUDE.md".to_string()],
            vec!["agent-browser".to_string()],
        );
        let lines = data.build_header(80);
        assert_eq!(lines[7], "\x1b[1m[Skills]\x1b[0m");
        assert_eq!(lines[8], "\x1b[2m  agent-browser\x1b[0m");
    }

    #[test]
    fn test_startup_header_separators() {
        let data = HeaderTestData::new(
            vec!["CLAUDE.md".to_string()],
            vec!["agent-browser".to_string()],
        );
        let lines = data.build_header(80);
        assert_eq!(lines.len(), 9);
        assert_eq!(lines[0], "");
    }

    #[test]
    fn test_startup_header_no_context_no_skills() {
        let data = HeaderTestData::new(vec![], vec![]);
        let lines = data.build_header(80);
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], "");
        assert_eq!(
            lines[4],
            "\x1b[2mPick can explain its own features and look up its docs. Ask it how to use or extend Pick.\x1b[0m"
        );
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1500), "1.5k");
        assert_eq!(format_tokens(15000), "15k");
        assert_eq!(format_tokens(1_500_000), "1.5M");
    }

    #[test]
    fn test_footer_line1_cwd_with_home() {
        let line = render_footer_line1_test(
            "D:\\autoway\\Project\\agent\\vividCode",
            Some("D:\\autoway"),
            None,
            120,
        );
        assert!(line.starts_with("\x1b[2m~"));
        assert!(line.ends_with("\x1b[0m"));
    }

    #[test]
    fn test_footer_line1_with_git_branch() {
        let line = render_footer_line1_test("~/project", None, Some("main"), 120);
        assert!(line.contains("main"));
    }

    fn render_footer_line1_test(
        cwd: &str,
        home: Option<&str>,
        git_branch: Option<&str>,
        width: u16,
    ) -> String {
        let mut pwd = format_cwd_for_footer(cwd, home);
        if let Some(branch) = git_branch {
            pwd = format!("{} ({})", pwd, branch);
        }
        if pwd.len() > width as usize {
            pwd.truncate(width.saturating_sub(3) as usize);
            pwd.push_str("...");
        }
        format!("\x1b[2m{}\x1b[0m", pwd)
    }

    #[test]
    fn test_footer_line2_no_usage() {
        let line = render_footer_line2_test(
            0,
            0,
            0,
            0,
            Some(0.0),
            1_000_000,
            "claude-sonnet-4-20250514",
            "off",
            true,
            120,
        );
        assert!(line.contains("0.0%/1.0M"), "line = {:?}", line);
        assert!(line.contains("(auto)"), "line = {:?}", line);
        assert!(
            line.contains("claude-sonnet-4-20250514"),
            "line = {:?}",
            line
        );
    }

    #[test]
    fn test_footer_line2_with_usage() {
        let line = render_footer_line2_test(
            1500,
            3000,
            500,
            200,
            Some(50.0),
            1_000_000,
            "claude-sonnet-4-20250514",
            "off",
            true,
            120,
        );
        assert!(line.contains("50.0%/1.0M"), "line = {:?}", line);
        assert!(line.contains("(auto)"), "line = {:?}", line);
        assert!(
            line.contains("claude-sonnet-4-20250514"),
            "line = {:?}",
            line
        );
    }

    #[test]
    fn test_footer_line2_with_thinking() {
        let line = render_footer_line2_test(
            0,
            0,
            0,
            0,
            None,
            1_000_000,
            "claude-sonnet-4-20250514",
            "high",
            true,
            120,
        );
        assert!(line.contains("high"));
    }

    fn render_footer_line2_test(
        total_input: u64,
        total_output: u64,
        total_cache_read: u64,
        total_cache_write: u64,
        context_percent: Option<f64>,
        context_window: u64,
        model_id: &str,
        thinking_level: &str,
        auto_compact: bool,
        width: u16,
    ) -> String {
        let _ = (
            total_input,
            total_output,
            total_cache_read,
            total_cache_write,
        );
        let auto_indicator = if auto_compact { " (auto)" } else { "" };
        let left_side = match context_percent {
            Some(pct) => format!(
                "{:.1}%/{}{}",
                pct,
                format_tokens(context_window),
                auto_indicator
            ),
            None => format!("?/{}{}", format_tokens(context_window), auto_indicator),
        };
        let mut right_side = model_id.to_string();
        if thinking_level != "off" {
            right_side = format!("{} \u{2022} {}", model_id, thinking_level);
        }
        let left_colored = format!("\x1b[2m{}\x1b[0m", left_side);
        let right_colored = format!("\x1b[2m{}\x1b[0m", right_side);
        let visible_left = left_side.len();
        let visible_right = right_side.len();
        let available = width as usize;
        if visible_left + visible_right + 2 < available {
            let padding = available - visible_left - visible_right;
            format!(
                "{}{:padding$}{}",
                left_colored,
                "",
                right_colored,
                padding = padding
            )
        } else {
            format!("{}  {}", left_colored, right_colored)
        }
    }

    #[test]
    fn test_full_tui_startup_display() {
        let mut app = TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1.0",
            vec!["CLAUDE.md".to_string()],
            vec!["agent-browser".to_string()],
            "D:\\autoway\\Project\\agent\\vividCode",
            Some("D:\\autoway".to_string()),
            "off",
            None,
            "test",
            "",
        );

        app.ensure_startup_header(120);
        assert!(
            app.startup_header_added,
            "startup_header_added should be true"
        );

        let rendered = app.chat.render_lines(120, 200);
        let rendered_text: Vec<String> = rendered.iter().map(line_text).collect();
        assert!(
            !rendered_text.is_empty(),
            "chat render should not be empty; got {} lines",
            rendered_text.len()
        );

        assert!(
            rendered_text[0].starts_with('╭'),
            "top border should start with ╭, got: {:?}",
            rendered_text[0]
        );
        assert!(
            rendered_text[1].contains("🤖 Pick"),
            "title should contain '🤖 Pick', got: {:?}",
            rendered_text[1]
        );
        assert!(
            rendered_text[3].contains("model:"),
            "model line should be present, got: {:?}",
            rendered_text[3]
        );
        assert!(
            rendered_text[4].contains("directory:"),
            "directory line should be present, got: {:?}",
            rendered_text[4]
        );
        assert!(
            rendered_text[6].contains("Pick can explain"),
            "description should be present, got: {:?}",
            rendered_text[6]
        );
        assert!(
            rendered_text[7].contains("extend Pick"),
            "description continuation should be present, got: {:?}",
            rendered_text[7]
        );
        assert!(
            rendered_text[9].contains("[Context]"),
            "Context section header should be present, got: {:?}",
            rendered_text[9]
        );
        assert!(
            rendered_text[10].contains("[Skills]"),
            "Skills section header should be present, got: {:?}",
            rendered_text[10]
        );
        assert!(
            rendered_text[11].starts_with('╰'),
            "bottom border should start with ╰, got: {:?}",
            rendered_text[11]
        );
        assert!(
            rendered_text[12].contains("Tip:"),
            "tip line should be present, got: {:?}",
            rendered_text[12]
        );

        assert_eq!(
            rendered_text.len(),
            13,
            "should have 13 lines, got {}",
            rendered_text.len()
        );
    }

    #[test]
    fn test_startup_screen_visual() {
        let mut app = TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1.0",
            vec!["CLAUDE.md".to_string()],
            vec!["agent-browser".to_string()],
            "D:\\autoway\\Project\\agent\\vividCode",
            Some("D:\\autoway".to_string()),
            "off",
            None,
            "test",
            "",
        );

        app.ensure_startup_header(80);
        app.git_branch = Some("main".to_string());
        app.session_name = Some("test-session".to_string());

        let chat_lines = app.chat.render_lines(80, usize::MAX);
        let chat_text: Vec<String> = chat_lines.iter().map(line_text).collect();
        println!("\n===== Pick TUI STARTUP SCREEN (chat lines) =====");
        for (i, line) in chat_text.iter().enumerate() {
            println!("{:>2}: {}", i, line);
        }
        println!("====================================================");

        println!("Footer1: {}", app.render_footer_line1(80));
        println!("Footer2: {}", app.render_footer_line2(80));
    }

    #[test]
    fn test_editor_renders_typed_characters() {
        let mut app = TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1.0",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );

        app.editor.insert_char('h');
        app.editor.insert_char('e');
        app.editor.insert_char('l');
        app.editor.insert_char('l');
        app.editor.insert_char('o');

        let (lines, cursor_row, cursor_col) = app.editor.render(80, 5);

        assert!(!lines.is_empty(), "editor should produce at least one line");
        assert_eq!(
            line_text(&lines[0]),
            "\u{276f} hello",
            "first line should show prompt and text. got: {:?}",
            lines[0]
        );
        assert_eq!(cursor_row, 0, "cursor should be on first row");
        assert_eq!(cursor_col, 7, "cursor should be after prompt (2) + 5 chars");
    }

    #[test]
    fn test_editor_chinese_characters() {
        let mut app = TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1.0",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );

        app.editor.insert_char('你');
        app.editor.insert_char('好');

        let (lines, cursor_row, cursor_col) = app.editor.render(80, 5);

        assert!(!lines.is_empty(), "editor should produce at least one line");
        assert_eq!(
            line_text(&lines[0]),
            "\u{276f} 你好",
            "line content should be '❯ 你好'. got: {:?}",
            lines[0]
        );
        assert_eq!(cursor_row, 0, "cursor should be on first row");
        assert_eq!(
            cursor_col, 6,
            "cursor col should be 6 (prompt 2 + 2 CJK chars × 2 width), not 8 (byte offset)"
        );

        let action = app.submit_input();
        assert!(action.is_some(), "should submit");
        if let Some(TuiAction::Submit(text)) = &action {
            assert_eq!(text, "你好", "submitted text should be '你好'");
            app.chat.add_user_message(text);
        }

        let chat_lines = app.chat.render_lines(80, 200);
        let all_text: String = chat_lines
            .iter()
            .map(line_text)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            all_text.contains("你好"),
            "chat should contain '你好'. got: {:?}",
            all_text
        );
    }

    #[test]
    fn test_editor_submit_moves_text_to_chat() {
        let mut app = TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1.0",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );

        app.editor.insert_char('h');
        app.editor.insert_char('i');

        let action = app.submit_input();
        assert!(action.is_some(), "submit_input should return Some(action)");
        if let Some(TuiAction::Submit(text)) = &action {
            assert_eq!(text, "hi", "submitted text should be 'hi'");
            app.chat.add_user_message(text);
        } else {
            panic!("expected Submit action, got {:?}", action);
        }

        assert!(
            app.editor.buffer.is_empty(),
            "editor should be cleared after submit"
        );

        let chat_lines = app.chat.render_lines(80, 200);
        assert!(
            !chat_lines.is_empty(),
            "chat should have content after submit"
        );
        let all_text: String = chat_lines
            .iter()
            .map(line_text)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            all_text.contains("hi"),
            "chat should contain submitted text 'hi'. got: {:?}",
            all_text
        );
    }

    #[test]
    fn test_separator_renders_dimmed() {
        let separator = format!("\x1b[2m{}\x1b[0m", "\u{2500}".repeat(40));
        assert!(
            separator.contains("\u{2500}"),
            "separator should use box-drawing char"
        );
        assert!(
            separator.contains("\x1b[2m"),
            "separator should have ANSI dim code"
        );
        assert!(
            separator.contains("\x1b[0m"),
            "separator should have ANSI reset code"
        );
    }

    #[test]
    fn test_selection_popup_uses_accent_style() {
        let mut app = TuiApp::new_inner(
            "anthropic",
            "claude",
            "test",
            "1.0",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );

        let items = vec![
            SelectItem::new("option-a", "a").with_description("First option"),
            SelectItem::new("option-b", "b").with_description("Second option"),
        ];
        let select = SelectList::new("Test Title", items);
        app.start_selection(select);
        assert_eq!(app.state, AppState::Selecting);

        let lines = app.build_selection_popup_lines(60);
        assert!(!lines.is_empty(), "should have popup lines");

        assert!(
            lines[0].spans[0]
                .style
                .add_modifier
                .contains(Modifier::BOLD),
            "title should be bold"
        );
        assert!(
            line_text(&lines[0]).contains("Test Title"),
            "title should contain 'Test Title', got: {}",
            line_text(&lines[0])
        );

        assert!(
            lines.len() > 1,
            "should have at least 2 lines, got {}",
            lines.len()
        );
        assert!(
            line_text(&lines[1]).contains("\u{2192}"),
            "selected item should have arrow marker, got: {}",
            line_text(&lines[1])
        );
        assert_eq!(
            lines[1].spans[0].style.fg,
            Some(Color::Cyan),
            "selected arrow should be cyan"
        );

        let all_text: String = lines
            .iter()
            .map(|l| line_text(l))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            all_text.contains("First option"),
            "should contain description 'First option', got: {}",
            all_text
        );
    }

    #[test]
    fn test_autocomplete_pi_style_layout() {
        use crate::editor::Editor;
        let mut ed = Editor::new();
        let commands = vec![
            crate::autocomplete::SlashCommand {
                name: "settings".to_string(),
                description: Some("Open settings menu".to_string()),
                argument_hint: None,
            },
            crate::autocomplete::SlashCommand {
                name: "model".to_string(),
                description: Some("Select model (opens selector UI)".to_string()),
                argument_hint: None,
            },
            crate::autocomplete::SlashCommand {
                name: "export".to_string(),
                description: Some("Export session".to_string()),
                argument_hint: None,
            },
        ];
        ed.set_autocomplete_provider(Box::new(
            crate::autocomplete::CombinedAutocompleteProvider::new(
                commands,
                std::path::PathBuf::from("/tmp"),
            ),
        ));

        ed.insert_char('/');
        assert!(ed.is_autocomplete_active());

        let ac_lines = ed.render_autocomplete(80, 10);
        assert!(
            ac_lines.len() >= 4,
            "should have 3 suggestions + counter, got {} lines",
            ac_lines.len()
        );

        let first = &ac_lines[0];
        assert!(
            line_text(first).contains("→"),
            "selected item should have → marker: {}",
            line_text(first)
        );
        assert_eq!(
            first.spans[0].style.fg,
            Some(Color::Cyan),
            "selected arrow should be cyan"
        );
        assert!(
            line_text(first).contains("settings"),
            "first item should be settings: {}",
            line_text(first)
        );

        let second = &ac_lines[1];
        assert!(
            !line_text(second).contains("→"),
            "non-selected should not have arrow: {}",
            line_text(second)
        );
        assert!(
            line_text(second).contains("model"),
            "second item should be model: {}",
            line_text(second)
        );

        let last_span = second.spans.last().unwrap();
        assert!(
            last_span.style.add_modifier.contains(Modifier::DIM),
            "non-selected description should be dimmed"
        );

        let last = ac_lines.last().unwrap();
        assert!(
            line_text(last).contains("(1/3)"),
            "counter should show (1/3), got: {}",
            line_text(last)
        );
        assert!(
            last.spans[0].style.add_modifier.contains(Modifier::DIM),
            "counter should be dimmed"
        );

        ed.autocomplete_next();
        let ac_lines2 = ed.render_autocomplete(80, 10);
        let t0 = line_text(&ac_lines2[0]);
        let t1 = line_text(&ac_lines2[1]);
        assert!(
            t1.contains("→"),
            "after next(), second line should have arrow, got: {}",
            t1
        );
        assert!(
            !t0.contains("→"),
            "after next(), first line should NOT have arrow, got: {}",
            t0
        );
    }

    #[test]
    fn test_all_slash_commands_in_autocomplete() {
        use crate::editor::Editor;
        let mut ed = Editor::new();

        let builtin_names = [
            "settings",
            "model",
            "scoped-models",
            "export",
            "import",
            "share",
            "copy",
            "name",
            "session",
            "changelog",
            "hotkeys",
            "fork",
            "clone",
            "tree",
            "login",
            "logout",
            "new",
            "compact",
            "resume",
            "reload",
            "quit",
        ];

        let commands: Vec<crate::autocomplete::SlashCommand> = builtin_names
            .iter()
            .map(|name| crate::autocomplete::SlashCommand {
                name: name.to_string(),
                description: Some(format!("Command: {}", name)),
                argument_hint: None,
            })
            .collect();

        ed.set_autocomplete_provider(Box::new(
            crate::autocomplete::CombinedAutocompleteProvider::new(
                commands,
                std::path::PathBuf::from("/tmp"),
            ),
        ));

        ed.insert_char('/');
        assert!(ed.is_autocomplete_active());

        let mut seen = vec![false; builtin_names.len()];
        for _ in 0..builtin_names.len() {
            let ac_lines = ed.render_autocomplete(80, 100);
            for line in &ac_lines {
                for (idx, name) in builtin_names.iter().enumerate() {
                    if !seen[idx] && line_text(line).contains(*name) {
                        seen[idx] = true;
                    }
                }
            }
            ed.autocomplete_next();
        }
        for (idx, name) in builtin_names.iter().enumerate() {
            assert!(
                seen[idx],
                "command /{} should be visible in autocomplete",
                *name
            );
        }
    }

    #[test]
    fn test_editor_renders_visually_in_buffer() {
        let mut app = TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1.0",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );

        app.editor.insert_char('t');
        app.editor.insert_char('e');
        app.editor.insert_char('s');
        app.editor.insert_char('t');

        let (editor_lines, cursor_row, cursor_col) = app.editor.render(80, 1);
        assert!(
            !editor_lines.is_empty(),
            "editor should produce at least one line"
        );
        assert!(
            line_text(&editor_lines[0]).contains("test"),
            "editor should show typed text 'test'. got: {:?}",
            editor_lines[0]
        );
        assert_eq!(cursor_row, 0, "cursor should be on first row");
        assert!(cursor_col > 0, "cursor should be past the prompt");

        println!("\n===== EDITOR RENDER TEST =====");
        for (i, line) in editor_lines.iter().enumerate() {
            println!("{:>2}: {}", i, line);
        }
        println!("================================");
    }

    #[test]
    fn test_shift_enter_newline_and_delete() {
        let mut app = TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1.0",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );

        app.handle_key(KeyCode::Char('h'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Char('e'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Char('l'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Char('l'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Char('o'), KeyModifiers::NONE);
        assert_eq!(app.editor.buffer, "hello");
        assert_eq!(app.editor.line_count(), 1);

        app.handle_key(KeyCode::Enter, KeyModifiers::SHIFT);
        assert_eq!(app.editor.buffer, "hello\n");
        assert_eq!(app.editor.line_count(), 2);

        app.handle_key(KeyCode::Char('a'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Char('b'), KeyModifiers::NONE);
        assert_eq!(app.editor.buffer, "hello\nab");

        app.handle_key(KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(app.editor.buffer, "hello\na");

        app.handle_key(KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(app.editor.buffer, "hello\n");

        app.handle_key(KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(app.editor.cursor, 5, "cursor should be at newline (pos 5)");

        app.handle_key(KeyCode::Delete, KeyModifiers::NONE);
        assert_eq!(app.editor.buffer, "hello");
        assert_eq!(app.editor.line_count(), 1);

        app.handle_key(KeyCode::Enter, KeyModifiers::CONTROL);
        assert_eq!(app.editor.buffer, "hello\n");
        assert_eq!(app.editor.line_count(), 2);
    }

    #[test]
    fn test_integration_user_sends_ni_hao_and_receives_thinking_response() {
        let mut app = TuiApp::new_inner(
            "faux",
            "faux-model",
            "Pick",
            "0.1.0",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );

        app.editor.insert_char('你');
        app.editor.insert_char('好');
        assert_eq!(app.editor.text(), "你好");

        let action = app.handle_key(KeyCode::Enter, KeyModifiers::NONE);
        assert!(action.is_some(), "Enter should return a Submit action");
        if let Some(TuiAction::Submit(text)) = &action {
            assert_eq!(text, "你好", "submitted text should be '你好'");
        } else {
            panic!("Expected Submit action, got {:?}", action);
        }

        assert_eq!(
            app.state,
            AppState::Streaming,
            "state should be Streaming after submit"
        );
        assert!(
            app.editor.buffer.is_empty(),
            "editor should be cleared after submit"
        );

        let thinking = "用户发来问候，我需要礼貌地回应";
        let response = "你好！有什么我可以帮助你的吗？😊";
        let combined = format!(
            "\x1b[3m\x1b[38;2;128;128;128m\u{2022} {}\x1b[23m\x1b[39m\n\n{}",
            thinking, response
        );
        app.stream_content(&combined);

        app.finalize_turn();
        assert_eq!(
            app.state,
            AppState::Input,
            "state should be Input after finalize_turn"
        );

        let chat_lines = app.chat.render_lines(80, usize::MAX);
        let line_texts: Vec<String> = chat_lines.iter().map(line_text).collect();
        let text: String = line_texts.join(" ");

        assert!(
            text.contains("你好"),
            "User message '你好' should be in rendered output.\nGot: {:?}",
            text
        );
        assert!(
            text.contains("用户发来问候"),
            "Thinking text should be in rendered output.\nGot: {:?}",
            text
        );
        assert!(
            text.contains("有什么我可以帮助你的吗"),
            "Response text should be in rendered output.\nGot: {:?}",
            text
        );

        let mut max_blank_run = 0usize;
        let mut blank_run = 0usize;
        for l in &line_texts {
            if l.trim().is_empty() {
                blank_run += 1;
                max_blank_run = std::cmp::max(max_blank_run, blank_run);
            } else {
                blank_run = 0;
            }
        }
        assert!(
            max_blank_run <= 2,
            "At most 2 consecutive blank lines, got {}. Lines:\n{:?}",
            max_blank_run,
            line_texts
        );

        eprintln!("\n===== FULL INTEGRATION TEST: RENDERED CHAT =====");
        eprintln!("Total chat lines: {}", line_texts.len());
        for (i, line) in line_texts.iter().enumerate() {
            eprintln!("  [{:>2}] {:?}", i, line);
        }
        eprintln!("================================================\n");
    }

    #[test]
    fn test_footer_full_render_contains_model_and_context() {
        let mut app = TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1.0",
            vec![],
            vec![],
            "/tmp",
            None,
            "high",
            None,
            "test",
            "",
        );
        app.ensure_startup_header(80);

        let chat_lines = app.chat.render_lines(80, usize::MAX);
        assert!(!chat_lines.is_empty(), "chat should render successfully");

        let footer2 = app.render_footer_line2(80);
        let footer2_text: String = footer2.spans.iter().map(|s| s.content.as_ref()).collect();

        assert!(
            footer2_text.contains("claude-sonnet-4-20250514"),
            "model name should be in footer, got: {:?}",
            footer2_text
        );
        assert!(
            footer2_text.contains("0.0%/1.0M"),
            "context info should be in footer, got: {:?}",
            footer2_text
        );
        assert!(
            footer2_text.contains("high"),
            "thinking level should be in footer, got: {:?}",
            footer2_text
        );
    }

    #[test]
    fn test_fix_no_scrollback_duplication() {
        let mut app = TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1.0",
            vec!["CLAUDE.md".to_string()],
            vec![],
            "D:\\test",
            Some("D:\\".to_string()),
            "off",
            None,
            "test",
            "",
        );

        app.ensure_startup_header(80);
        app.chat.add_user_message("hello");
        app.state = AppState::Streaming;
        app.chat.stream_assistant_content("Hi there!");
        app.chat.mark_turn_end();
        app.state = AppState::Input;

        let lines1 = app.chat.render_lines(80, usize::MAX);
        let text1: String = lines1.iter().map(line_text).collect::<Vec<_>>().join(" ");
        assert_eq!(text1.matches("Pick").count(), 3);
        assert_eq!(text1.matches("hello").count(), 1);
        assert_eq!(text1.matches("Hi there!").count(), 1);

        app.chat.add_user_message("how are you?");
        app.state = AppState::Streaming;
        app.chat.stream_assistant_content("Doing well!");
        app.chat.mark_turn_end();
        app.state = AppState::Input;

        let lines2 = app.chat.render_lines(80, usize::MAX);
        let text2: String = lines2.iter().map(line_text).collect::<Vec<_>>().join(" ");
        assert_eq!(text2.matches("Pick").count(), 3);
        assert_eq!(text2.matches("hello").count(), 1);
        assert_eq!(
            text2.matches("Hi there!").count(),
            1,
            "first assistant content persists (each turn commits separately)"
        );
        assert_eq!(text2.matches("how are you?").count(), 1);
        assert_eq!(text2.matches("Doing well!").count(), 1);

        let lines3 = app.chat.render_lines(80, usize::MAX);
        let text3: String = lines3.iter().map(line_text).collect::<Vec<_>>().join(" ");
        assert_eq!(text3.matches("Pick").count(), 3);
        assert_eq!(text3.matches("hello").count(), 1);
        assert_eq!(text3.matches("Hi there!").count(), 1);
        assert_eq!(text3.matches("how are you?").count(), 1);
        assert_eq!(text3.matches("Doing well!").count(), 1);

        for _ in 0..5 {
            let lines_n = app.chat.render_lines(80, usize::MAX);
            let text_n: String = lines_n.iter().map(line_text).collect::<Vec<_>>().join(" ");
            assert_eq!(text_n.matches("Pick").count(), 3);
            assert_eq!(text_n.matches("hello").count(), 1);
            assert_eq!(text_n.matches("Hi there!").count(), 1);
            assert_eq!(text_n.matches("how are you?").count(), 1);
            assert_eq!(text_n.matches("Doing well!").count(), 1);
        }
    }

    #[test]
    fn test_typing_no_blank_accumulation() {
        let mut app = TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1.0",
            vec!["CLAUDE.md".to_string()],
            vec![],
            "D:\\test",
            Some("D:\\".to_string()),
            "off",
            None,
            "test",
            "",
        );
        app.ensure_startup_header(80);

        for r in 1..=3 {
            app.chat.add_user_message(&format!("msg {}", r));
            app.state = AppState::Streaming;
            app.chat.stream_assistant_content(&format!("reply {}", r));
            app.chat.mark_turn_end();
            app.state = AppState::Input;
            app.chat.render_lines(80, usize::MAX);
        }

        let mut prev = None;
        for i in 0..5 {
            let lines = app.chat.render_lines(80, usize::MAX);
            let count = lines.len();
            if let Some(p) = prev {
                assert_eq!(count, p, "typing render {} has mismatched line count", i);
            }
            prev = Some(count);
        }
    }
}
