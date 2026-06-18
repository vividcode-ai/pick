pub mod bash;
pub mod edit;
pub mod edit_diff;
pub mod file_mutation_queue;
pub mod find;
pub mod grep;
pub mod ls;
pub mod output_accumulator;
pub mod path_utils;
pub mod plan_tools;
pub mod read;
pub mod render_utils;
pub mod tool_definition_wrapper;
pub mod truncate;
pub mod write;

pub use render_utils::*;

/// Render a tool call by name, dispatching to the appropriate tool render function.
/// Returns None if the tool has no renderer.
pub fn render_tool_call(
    tool_name: &str,
    args: &serde_json::Value,
    ctx: &ToolRenderContext,
) -> Option<ToolRenderOutput> {
    match tool_name {
        "read" => Some(read::render_read_call(args, ctx)),
        "bash" => Some(bash::render_bash_call(args, ctx)),
        "write" => Some(write::render_write_call(args, ctx)),
        "edit" => Some(edit::render_edit_call(args, ctx)),
        "grep" => Some(grep::render_grep_call(args, ctx)),
        "find" => Some(find::render_find_call(args, ctx)),
        "ls" => Some(ls::render_ls_call(args, ctx)),
        _ => None,
    }
}

/// All tool names supported by the system
pub const ALL_TOOL_NAMES: &[&str] = &["read", "bash", "edit", "write", "grep", "find", "ls"];

/// Tool name type
pub type ToolName = &'static str;
