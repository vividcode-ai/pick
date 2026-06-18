use super::edit_diff::*;
use super::file_mutation_queue::with_file_mutation_queue;
use super::path_utils::resolve_to_cwd;
use super::render_utils::{
    ToolRenderContext, ToolRenderOptions, ToolRenderOutput, ToolTheme, invalid_arg_text,
    shorten_path,
};

/// Create an edit tool definition
pub fn create_edit_tool_definition() -> EditToolDefinition {
    EditToolDefinition
}

pub struct EditToolDefinition;

impl EditToolDefinition {
    pub fn name(&self) -> &str {
        "edit"
    }

    pub fn description(&self) -> &str {
        "Edit a single file using exact text replacement."
    }

    pub async fn execute(
        &self,
        path: &str,
        edits: &[Edit],
        cwd: &str,
    ) -> Result<EditOutput, String> {
        let absolute_path = resolve_to_cwd(path, cwd);

        with_file_mutation_queue(&absolute_path, async || {
            // Check if file exists
            if !tokio::fs::try_exists(&absolute_path)
                .await
                .map_err(|e| e.to_string())?
            {
                return Err(format!("Could not edit file: {}. File not found.", path));
            }

            // Read the file
            let raw_content = tokio::fs::read_to_string(&absolute_path)
                .await
                .map_err(|e| format!("Could not read file: {}", e))?;

            // Strip BOM
            let (bom, text) = strip_bom(&raw_content);
            let original_ending = detect_line_ending(&text);
            let normalized_content = normalize_to_lf(&text);

            let result = apply_edits_to_normalized_content(&normalized_content, edits, path)?;

            let final_content = format!(
                "{}{}",
                bom,
                restore_line_endings(&result.new_content, original_ending)
            );
            tokio::fs::write(&absolute_path, &final_content)
                .await
                .map_err(|e| format!("Failed to write file: {}", e))?;

            let diff_result = generate_diff_string(&result.base_content, &result.new_content, 4);

            Ok(EditOutput {
                content: vec![serde_json::json!({
                    "type": "text",
                    "text": format!("Successfully replaced {} block(s) in {}.", edits.len(), path)
                })],
                details: EditToolDetails {
                    diff: diff_result.diff.clone(),
                    first_changed_line: diff_result.first_changed_line,
                },
            })
        })
        .await
    }
}

pub struct EditOutput {
    pub content: Vec<serde_json::Value>,
    pub details: EditToolDetails,
}

pub struct EditToolDetails {
    pub diff: String,
    pub first_changed_line: Option<usize>,
}

// ============================================================================
// Render Functions
// ============================================================================

/// Render an edit tool call — `edit /path/to/file` with diff preview
pub fn render_edit_call(args: &serde_json::Value, _ctx: &ToolRenderContext) -> ToolRenderOutput {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .or_else(|| args.get("file_path").and_then(|v| v.as_str()));

    let path_display = match path {
        Some(p) => ToolTheme::fg("accent", &shorten_path(p)),
        None => invalid_arg_text(&|s| ToolTheme::fg("error", s)),
    };

    let label = format!(
        "{} {}",
        ToolTheme::fg("toolTitle", &ToolTheme::bold("edit")),
        path_display,
    );

    ToolRenderOutput {
        label,
        formatted: String::new(),
    }
}

/// Render an edit tool result — diff output or error
pub fn render_edit_result(
    output: &EditOutput,
    _options: &ToolRenderOptions,
    ctx: &ToolRenderContext,
) -> ToolRenderOutput {
    if ctx.is_error {
        let error_text: String = output
            .content
            .iter()
            .filter_map(|c| c.get("text").and_then(|v| v.as_str()))
            .collect::<Vec<_>>()
            .join("\n");
        if !error_text.is_empty() {
            return ToolRenderOutput {
                label: String::new(),
                formatted: format!("\n{}", ToolTheme::fg("error", &error_text)),
            };
        }
        return ToolRenderOutput {
            label: String::new(),
            formatted: String::new(),
        };
    }

    let diff = &output.details.diff;
    if diff.is_empty() {
        return ToolRenderOutput {
            label: String::new(),
            formatted: String::new(),
        };
    }

    ToolRenderOutput {
        label: String::new(),
        formatted: format!("\n{}", diff),
    }
}
