//! Input validation utilities

use serde_json::Value;

/// Validate that a value matches a JSON schema
pub fn validate_against_schema(value: &Value, schema: &crate::types::JsonSchema) -> Result<(), Vec<String>> {
    let schema_value = serde_json::to_value(schema).map_err(|e| vec![format!("Failed to serialize schema: {}", e)])?;

    let validator = match jsonschema::validator_for(&schema_value) {
        Ok(v) => v,
        Err(e) => return Err(vec![format!("Invalid schema: {}", e)]),
    };

    if validator.is_valid(value) {
        return Ok(());
    }

    let error_messages: Vec<String> = validator
        .iter_errors(value)
        .map(|e| format!("{}: {}", e.instance_path, e))
        .collect();
    Err(error_messages)
}
