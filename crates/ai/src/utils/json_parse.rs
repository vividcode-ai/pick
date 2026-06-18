//! JSON parsing utilities (safe parsing with error handling)

/// Safely parse a JSON string, returning None on failure
pub fn safe_parse_json(input: &str) -> Option<serde_json::Value> {
    serde_json::from_str(input).ok()
}

/// Try to extract a JSON object from text (handles markdown code fences)
pub fn extract_json_from_text(text: &str) -> Option<serde_json::Value> {
    // Try direct parse first
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(text) {
        return Some(val);
    }

    // Try extracting from ```json ... ``` blocks
    if let Some(start) = text.find("```json") {
        let after_start = &text[start + 7..];
        if let Some(end) = after_start.find("```") {
            let candidate = after_start[..end].trim();
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(candidate) {
                return Some(val);
            }
        }
    }

    // Try extracting from ``` ... ``` blocks
    if let Some(start) = text.find("```") {
        let after_start = &text[start + 3..];
        if let Some(end) = after_start.find("```") {
            let candidate = after_start[..end].trim();
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(candidate) {
                return Some(val);
            }
        }
    }

    None
}

/// Extract a string field from a JSON value
pub fn get_json_string<'a>(val: &'a serde_json::Value, field: &'a str) -> Option<&'a str> {
    val.get(field)?.as_str()
}

/// Extract a u64 field from a JSON value
pub fn get_json_u64(val: &serde_json::Value, field: &str) -> Option<u64> {
    val.get(field)?.as_u64()
}
