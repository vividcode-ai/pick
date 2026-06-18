//! TypeBox-style JSON Schema helpers

use serde_json::Value;

/// Create a JSON Schema string enum from a list of string values.
/// Compatible with Google's API and other providers that don't support anyOf/const patterns.
///
/// # Example
///
/// ```
/// use pick_ai::utils::typebox_helpers::string_enum;
/// let schema = string_enum(&["add", "subtract", "multiply", "divide"], Some("The operation to perform"), None);
/// assert_eq!(schema["type"], "string");
/// assert_eq!(schema["enum"].as_array().unwrap().len(), 4);
/// ```
pub fn string_enum(
    values: &[&str],
    description: Option<&str>,
    default_value: Option<&str>,
) -> Value {
    let mut schema = serde_json::json!({
        "type": "string",
        "enum": values,
    });
    if let Some(desc) = description {
        schema["description"] = Value::String(desc.to_string());
    }
    if let Some(default) = default_value {
        schema["default"] = Value::String(default.to_string());
    }
    schema
}
