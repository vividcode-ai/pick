pub(crate) mod styles;
pub(crate) mod writer;

use std::path::PathBuf;

use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use ratatui::text::Text;

pub fn render_markdown_text(input: &str) -> Text<'static> {
    render_markdown_text_with_width_and_cwd(input, None, None)
}

pub(crate) fn render_markdown_text_with_width(input: &str, width: Option<usize>) -> Text<'static> {
    render_markdown_text_with_width_and_cwd(input, width, None)
}

pub(crate) fn render_markdown_text_with_width_and_cwd(
    input: &str,
    width: Option<usize>,
    cwd: Option<PathBuf>,
) -> Text<'static> {
    let mut w = writer::Writer::new(width, cwd);

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

    let parser = Parser::new_ext(input, options);

    for event in parser {
        w.handle_event(&event);
    }

    if w.has_table_state() {
        w.finalize_table();
    }

    w.text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_basic_text() {
        let result = render_markdown_text("hello world");
        assert!(!result.lines.is_empty());
        assert!(result.lines.iter().any(|l| {
            let s: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
            s.contains("hello")
        }));
    }

    #[test]
    fn renders_heading() {
        let result = render_markdown_text("# Heading 1");
        let joined: String = result
            .lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(joined.contains("Heading 1"), "got: {joined}");
    }

    #[test]
    fn renders_code_block() {
        let result = render_markdown_text("```rust\nfn main() {}\n```");
        assert!(!result.lines.is_empty());
        let joined: String = result
            .lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(joined.contains("fn main()"), "got: {joined}");
    }

    #[test]
    fn renders_table() {
        let md = "| H1 | H2 |\n|----|----|\n| A  | B  |\n| C  | D  |";
        let result = render_markdown_text(md);
        let joined: String = result
            .lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(joined.contains("H1"), "got: {joined}");
        assert!(joined.contains("A"), "got: {joined}");
        assert!(joined.contains("C"), "got: {joined}");
    }
}
