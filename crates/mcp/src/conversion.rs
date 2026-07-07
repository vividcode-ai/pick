use pick_agent::core::state::AgentToolResult;
use pick_ai::types::{ContentBlock, JsonSchema};

/// Convert a JSON Schema (from MCP `input_schema`) to Pick's `JsonSchema`.
/// MCP uses JSON Schema format, we extract the fields we need.
pub fn json_schema_from_mcp(
    input_schema: &serde_json::Map<String, serde_json::Value>,
) -> JsonSchema {
    let schema_type = input_schema
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("object")
        .to_string();

    let properties = input_schema
        .get("properties")
        .and_then(|v| v.as_object())
        .cloned();

    let required = input_schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

    let description = input_schema
        .get("description")
        .and_then(|v| v.as_str())
        .map(String::from);

    JsonSchema {
        schema_type,
        properties,
        required,
        description,
        items: None,
        additional_properties: Some(true),
    }
}

/// Generate tool guidelines from MCP tool schema.
/// Extracts required parameters and enum constraints.
pub fn generate_tool_guidelines(
    _tool_name: &str,
    input_schema: &serde_json::Map<String, serde_json::Value>,
) -> Vec<String> {
    let mut guidelines = Vec::new();

    // Extract required parameters info
    if let Some(required) = input_schema.get("required").and_then(|v| v.as_array()) {
        if !required.is_empty() {
            let req_names: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
            guidelines.push(format!("Required parameters: {}", req_names.join(", ")));
        }
    }

    // Extract enum constraints for string properties
    if let Some(properties) = input_schema.get("properties").and_then(|v| v.as_object()) {
        for (prop_name, prop_schema) in properties {
            if let Some(enum_vals) = prop_schema.get("enum").and_then(|v| v.as_array()) {
                if !enum_vals.is_empty() {
                    let enum_strs: Vec<&str> =
                        enum_vals.iter().filter_map(|v| v.as_str()).collect();
                    if !enum_strs.is_empty() {
                        guidelines.push(format!(
                            "`{}` accepts: {}",
                            prop_name,
                            enum_strs.join(" | ")
                        ));
                    }
                }
            }
        }
    }

    guidelines
}

/// Generate a one-line prompt snippet from tool name and schema
pub fn generate_prompt_snippet(
    tool_name: &str,
    input_schema: &serde_json::Map<String, serde_json::Value>,
) -> String {
    let props = input_schema
        .get("properties")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.keys()
                .map(|k| {
                    let desc = obj[k]
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or(k);
                    format!("{}: {}", k, desc)
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();

    format!("{}({})", tool_name, props)
}

/// Convert MCP `CallToolResult` content to Pick `AgentToolResult`.
pub fn mcp_result_to_agent_result(
    is_error: bool,
    content: &[rmcp::model::Content],
) -> AgentToolResult {
    let mut blocks: Vec<ContentBlock> = Vec::new();

    for item in content {
        match &item.raw {
            rmcp::model::RawContent::Text(t) => {
                blocks.push(ContentBlock::text(&t.text));
            }
            rmcp::model::RawContent::Image(img) => {
                blocks.push(ContentBlock::image(&img.data, &img.mime_type));
            }
            rmcp::model::RawContent::Resource(r) => match &r.resource {
                rmcp::model::ResourceContents::TextResourceContents { text, .. } => {
                    blocks.push(ContentBlock::text(text));
                }
                rmcp::model::ResourceContents::BlobResourceContents {
                    blob, mime_type, ..
                } => {
                    blocks.push(ContentBlock::image(
                        blob,
                        mime_type.as_deref().unwrap_or("application/octet-stream"),
                    ));
                }
            },
            rmcp::model::RawContent::Audio(a) => {
                blocks.push(ContentBlock::text(format!("[Audio: {}]", a.mime_type)));
            }
            rmcp::model::RawContent::ResourceLink(r) => {
                blocks.push(ContentBlock::text(format!(
                    "[Resource: {} ({})]",
                    r.uri, r.name
                )));
            }
        }
    }

    AgentToolResult {
        content: blocks,
        is_error,
        terminate: false,
    }
}
