//! Tool HTML renderer for custom tools in HTML export.

//!
//! Renders custom tool calls and results to HTML by invoking tool render functions
//! and converting the ANSI output to HTML.

use super::ansi_to_html;
use crate::core::tools::{ToolRenderContext, render_tool_call};

/// Pre-rendered HTML for a tool call and result
#[derive(Debug, Clone)]
pub struct RenderedToolHtml {
    pub call_html: Option<String>,
    pub result_html_collapsed: Option<String>,
    pub result_html_expanded: Option<String>,
}

/// Render a tool call to HTML.
/// Returns Some(HTML) if the tool has a render function, None otherwise.
pub fn render_tool_call_to_html(tool_name: &str, args: &serde_json::Value) -> Option<String> {
    let ctx = ToolRenderContext {
        args: Some(args.clone()),
        cwd: String::new(),
        expanded: false,
        show_images: false,
        is_error: false,
    };

    let output = render_tool_call(tool_name, args, &ctx)?;

    if output.label.is_empty() {
        return None;
    }

    let html = ansi_to_html::ansi_to_html(&output.label);
    Some(format!(r#"<div class="ansi-line">{}</div>"#, html))
}

/// Render a tool result to HTML (both collapsed and expanded views).
/// Returns None if the tool has no render function.
pub fn render_tool_result_to_html(
    tool_name: &str,
    _content: &[serde_json::Value],
    _details: Option<&serde_json::Value>,
    is_error: bool,
) -> Option<RenderedToolHtml> {
    // Build a render context for result rendering
    // For the basic export renderer, we produce simple HTML wrapping
    let ctx = ToolRenderContext {
        args: None,
        cwd: String::new(),
        expanded: false,
        show_images: false,
        is_error,
    };

    // Use the render call output as the "call" label and render result content generically
    // Since render_result functions take tool-specific types, we use a generic approach here:
    // render the call label for collapsed view, and the expanded context for expanded view.

    // For the collapsed result, render the tool label
    let collapsed_output = render_tool_call(tool_name, &serde_json::json!({}), &ctx);
    let collapsed = collapsed_output
        .filter(|o| !o.label.is_empty())
        .map(|o| html_line(&o.label));

    // For expanded, we just produce an empty container (actual rendering needs tool-specific types)
    let expanded = Some(String::new());

    match (collapsed, expanded) {
        (Some(c), e) => Some(RenderedToolHtml {
            call_html: None,
            result_html_collapsed: Some(c),
            result_html_expanded: e,
        }),
        _ => None,
    }
}

fn html_line(text: &str) -> String {
    if text.is_empty() {
        r#"<div class="ansi-line">&nbsp;</div>"#.to_string()
    } else {
        format!(
            "<div class=\"ansi-line\">{}</div>",
            ansi_to_html::ansi_to_html(text)
        )
    }
}
