//! Tool validation and parameter formatting

use super::super::state::AgentTool;

/// Validate tool arguments against the tool's parameter schema.
/// Ensures required params are present and attempts type coercion.
pub fn fmt_tool_params(tool: &AgentTool) -> String {
    let mut s = String::new();
    if let Some(props) = &tool.parameters.properties {
        let required = tool.parameters.required.as_deref().unwrap_or(&[]);
        for (key, schema) in props {
            let t = schema.get("type").and_then(|v| v.as_str()).unwrap_or("any");
            let req = if required.contains(key) { "required" } else { "optional" };
            if !s.is_empty() { s.push_str(", "); }
            s.push_str(&format!("{} ({}, {})", key, t, req));
        }
    }
    s
}

pub fn validate_tool_arguments(
    tool: &AgentTool,
    args: &serde_json::Value,
    raw_args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let params = &tool.parameters;
    let properties = params.properties.as_ref();

    // Normalize parameter aliases: copy file_path → path if path is missing
    let mut coerced = args.clone();
    if let Some(obj) = coerced.as_object_mut() {
        if !obj.contains_key("path") {
            if let Some(fp) = obj.get("file_path") {
                obj.insert("path".to_string(), fp.clone());
            }
        }
    }

    // Check required fields
    if let Some(required) = &params.required {
        for field in required {
            if !coerced.get(field).map_or(false, |v| !v.is_null()) {
                let details = fmt_tool_params(tool);
                return Err(format!(
                    "Tool '{}' requires a '{}' argument. Received args: {}. Parameters: {}",
                    tool.name, field, raw_args, details
                ));
            }
        }
    }

    // Attempt type coercion for known properties
    if let Some(props) = properties {
        if let Some(obj) = coerced.as_object_mut() {
            for (key, schema) in props {
                if let Some(expected_type) = schema.get("type").and_then(|t| t.as_str()) {
                    if let Some(val) = obj.get(key) {
                        let coerced_val = match (expected_type, val) {
                            ("number", serde_json::Value::String(s)) => {
                                s.parse::<f64>().ok().map(serde_json::Value::from)
                            }
                            ("integer", serde_json::Value::String(s)) => {
                                s.parse::<i64>().ok().map(serde_json::Value::from)
                            }
                            ("boolean", serde_json::Value::String(s)) => {
                                match s.as_str() {
                                    "true" => Some(serde_json::Value::Bool(true)),
                                    "false" => Some(serde_json::Value::Bool(false)),
                                    _ => None,
                                }
                            }
                            _ => None,
                        };
                        if let Some(cv) = coerced_val {
                            obj.insert(key.clone(), cv);
                        }
                    }
                }
            }
        }
    }

    Ok(coerced)
}
