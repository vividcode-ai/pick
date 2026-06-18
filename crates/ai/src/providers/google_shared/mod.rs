//! Shared utilities for Google Generative AI and Google Vertex providers.

pub mod types;
pub use types::*;

use serde::Serialize;
use serde_json::Value;

use crate::providers::transform_messages::transform_messages;
use crate::types::{
    AssistantMessage, Capability, ContentBlock, Context, Message, Model, StopReason, ToolDefinition,
};

/// Determines whether a streamed Gemini Part should be treated as "thinking".
pub fn is_thinking_part(thought: Option<bool>) -> bool {
    thought == Some(true)
}

/// Retain thought signatures during streaming.
pub fn retain_thought_signature(
    existing: Option<String>,
    incoming: Option<String>,
) -> Option<String> {
    match incoming {
        Some(ref s) if !s.is_empty() => incoming,
        _ => existing,
    }
}

fn is_valid_thought_signature(signature: Option<&str>) -> bool {
    let sig = match signature {
        Some(s) => s,
        None => return false,
    };
    if sig.len() % 4 != 0 {
        return false;
    }
    sig.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
}

fn resolve_thought_signature(
    is_same_provider_and_model: bool,
    signature: Option<&str>,
) -> Option<String> {
    if is_same_provider_and_model && is_valid_thought_signature(signature) {
        signature.map(|s| s.to_string())
    } else {
        None
    }
}

/// Models via Google APIs that require explicit tool call IDs.
pub fn requires_tool_call_id(model_id: &str) -> bool {
    model_id.starts_with("claude-") || model_id.starts_with("gpt-oss-")
}

fn get_gemini_major_version(model_id: &str) -> Option<u32> {
    let lower = model_id.to_lowercase();
    let re = regex::Regex::new(r"^gemini(?:-live)?-(\d+)").ok()?;
    let cap = re.captures(&lower)?;
    cap.get(1)?.as_str().parse().ok()
}

fn supports_multimodal_function_response(model_id: &str) -> bool {
    match get_gemini_major_version(model_id) {
        Some(v) => v >= 3,
        None => true,
    }
}

/// A Google API Content part (simplified representation).
#[derive(Debug, Clone, Serialize, Default)]
pub struct GooglePart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_data: Option<GoogleInlineData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<GoogleFunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_response: Option<GoogleFunctionResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GoogleInlineData {
    pub mime_type: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GoogleFunctionCall {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GoogleFunctionResponse {
    pub name: String,
    pub response: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parts: Option<Vec<GooglePart>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// A Google API Content entry.
#[derive(Debug, Clone, Serialize)]
pub struct GoogleContent {
    pub role: String,
    pub parts: Vec<GooglePart>,
}

/// Google format tool (function declarations).
#[derive(Debug, Clone, Serialize)]
pub struct GoogleTool {
    pub function_declarations: Vec<GoogleFunctionDeclaration>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GoogleFunctionDeclaration {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters_json_schema: Option<Value>,
}

fn convert_user_message_to_google(u: &crate::types::UserMessage) -> Option<GoogleContent> {
    let mut parts: Vec<GooglePart> = Vec::new();
    for block in &u.content {
        match block {
            ContentBlock::Text(t) => {
                let sanitized = crate::utils::sanitize_unicode::sanitize_unicode(&t.text);
                parts.push(GooglePart {
                    text: Some(sanitized),
                    ..Default::default()
                });
            }
            ContentBlock::Image(img) => {
                parts.push(GooglePart {
                    inline_data: Some(GoogleInlineData {
                        mime_type: img.mime_type.clone(),
                        data: img.data.clone(),
                    }),
                    ..Default::default()
                });
            }
            _ => {}
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(GoogleContent {
            role: "user".to_string(),
            parts,
        })
    }
}

fn convert_assistant_message_to_google(
    a: &AssistantMessage,
    model: &Model,
) -> Option<GoogleContent> {
    let is_same_provider_and_model = a.provider == model.provider.as_str() && a.model == model.id;

    let mut parts: Vec<GooglePart> = Vec::new();
    for block in &a.content {
        match block {
            ContentBlock::Text(t) => {
                if t.text.trim().is_empty() {
                    continue;
                }
                let thought_sig = resolve_thought_signature(
                    is_same_provider_and_model,
                    t.text_signature.as_deref(),
                );
                let sanitized = crate::utils::sanitize_unicode::sanitize_unicode(&t.text);
                parts.push(GooglePart {
                    text: Some(sanitized),
                    thought_signature: thought_sig,
                    ..Default::default()
                });
            }
            ContentBlock::Thinking(th) => {
                if th.thinking.trim().is_empty() {
                    continue;
                }
                let sanitized = crate::utils::sanitize_unicode::sanitize_unicode(&th.thinking);
                if is_same_provider_and_model {
                    let thought_sig =
                        resolve_thought_signature(true, th.thinking_signature.as_deref());
                    parts.push(GooglePart {
                        thought: Some(true),
                        text: Some(sanitized),
                        thought_signature: thought_sig,
                        ..Default::default()
                    });
                } else {
                    parts.push(GooglePart {
                        text: Some(sanitized),
                        ..Default::default()
                    });
                }
            }
            ContentBlock::ToolCall(tc) => {
                let thought_sig = resolve_thought_signature(
                    is_same_provider_and_model,
                    tc.thought_signature.as_deref(),
                );
                let mut fc = GoogleFunctionCall {
                    name: tc.name.clone(),
                    args: Some(tc.arguments.clone()),
                    id: None,
                };
                if requires_tool_call_id(&model.id) {
                    fc.id = Some(tc.id.clone());
                }
                parts.push(GooglePart {
                    function_call: Some(fc),
                    thought_signature: thought_sig,
                    ..Default::default()
                });
            }
            _ => {}
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(GoogleContent {
            role: "model".to_string(),
            parts,
        })
    }
}

fn convert_tool_result_to_google(
    tr: &crate::types::ToolResultMessage,
    model: &Model,
    contents: &mut Vec<GoogleContent>,
) {
    let text_content: Vec<String> = tr
        .content
        .iter()
        .filter_map(|c| {
            if let ContentBlock::Text(t) = c {
                Some(t.text.clone())
            } else {
                None
            }
        })
        .collect();
    let text_result = text_content.join("\n");

    let has_images = model
        .input_capabilities
        .iter()
        .any(|cap| cap == &Capability::Image)
        && tr
            .content
            .iter()
            .any(|c| matches!(c, ContentBlock::Image(_)));

    let model_supports_multimodal = supports_multimodal_function_response(&model.id);

    let response_value = if !text_result.is_empty() {
        crate::utils::sanitize_unicode::sanitize_unicode(&text_result)
    } else if has_images {
        "(see attached image)".to_string()
    } else {
        String::new()
    };

    let image_parts: Vec<GooglePart> = if has_images {
        tr.content
            .iter()
            .filter_map(|c| {
                if let ContentBlock::Image(img) = c {
                    Some(GooglePart {
                        inline_data: Some(GoogleInlineData {
                            mime_type: img.mime_type.clone(),
                            data: img.data.clone(),
                        }),
                        ..Default::default()
                    })
                } else {
                    None
                }
            })
            .collect()
    } else {
        vec![]
    };

    let include_id = requires_tool_call_id(&model.id);
    let function_response_part = GooglePart {
        function_response: Some(GoogleFunctionResponse {
            name: tr.tool_name.clone(),
            response: if tr.is_error {
                serde_json::json!({"error": response_value})
            } else {
                serde_json::json!({"output": response_value})
            },
            parts: if has_images && model_supports_multimodal && !image_parts.is_empty() {
                Some(image_parts.clone())
            } else {
                None
            },
            id: if include_id {
                Some(tr.tool_call_id.clone())
            } else {
                None
            },
        }),
        ..Default::default()
    };

    let should_merge = contents
        .last()
        .map(|c| c.role == "user" && c.parts.iter().any(|p| p.function_response.is_some()))
        .unwrap_or(false);

    if should_merge {
        if let Some(last) = contents.last_mut() {
            last.parts.push(function_response_part);
        }
    } else {
        contents.push(GoogleContent {
            role: "user".to_string(),
            parts: vec![function_response_part],
        });
    }

    if has_images && !model_supports_multimodal {
        let mut extra_parts = vec![GooglePart {
            text: Some("Tool result image:".to_string()),
            ..Default::default()
        }];
        extra_parts.extend(image_parts);
        contents.push(GoogleContent {
            role: "user".to_string(),
            parts: extra_parts,
        });
    }
}

/// Convert internal messages to Google Content[] format.
pub fn convert_messages(model: &Model, context: &Context) -> Vec<GoogleContent> {
    let mut contents: Vec<GoogleContent> = Vec::new();
    let model_id = &model.id;

    let needs_id_normalization = requires_tool_call_id(model_id);
    let normalize_id_closure = |id: &str, _model: &Model, _msg: &AssistantMessage| -> String {
        id.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .take(64)
            .collect()
    };
    let normalize_tool_call_id: Option<&dyn Fn(&str, &Model, &AssistantMessage) -> String> =
        if needs_id_normalization {
            Some(&normalize_id_closure)
        } else {
            None
        };

    let transformed = transform_messages(&context.messages, model, normalize_tool_call_id);

    for msg in &transformed {
        match msg {
            Message::User(u) => {
                if let Some(content) = convert_user_message_to_google(u) {
                    contents.push(content);
                }
            }
            Message::Assistant(a) => {
                if let Some(content) = convert_assistant_message_to_google(a, model) {
                    contents.push(content);
                }
            }
            Message::ToolResult(tr) => {
                convert_tool_result_to_google(tr, model, &mut contents);
            }
        }
    }

    contents
}

const JSON_SCHEMA_META_DECLARATIONS: &[&str] = &[
    "$schema",
    "$id",
    "$anchor",
    "$dynamicAnchor",
    "$vocabulary",
    "$comment",
    "$defs",
    "definitions",
];

fn sanitize_for_openapi(schema: &Value) -> Value {
    match schema {
        Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for (key, value) in map {
                if JSON_SCHEMA_META_DECLARATIONS.contains(&key.as_str()) {
                    continue;
                }
                result.insert(key.clone(), sanitize_for_openapi(value));
            }
            Value::Object(result)
        }
        _ => schema.clone(),
    }
}

/// Convert tools to Gemini function declarations format.
pub fn convert_tools(tools: &[ToolDefinition], use_parameters: bool) -> Option<Vec<GoogleTool>> {
    if tools.is_empty() {
        return None;
    }
    Some(vec![GoogleTool {
        function_declarations: tools
            .iter()
            .map(|t| {
                let params_value = serde_json::to_value(&t.parameters).unwrap_or_default();
                let params = if use_parameters {
                    Some(sanitize_for_openapi(&params_value))
                } else {
                    None
                };
                GoogleFunctionDeclaration {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: params,
                    parameters_json_schema: if use_parameters {
                        None
                    } else {
                        Some(params_value)
                    },
                }
            })
            .collect(),
    }])
}

/// Map tool choice string to Gemini FunctionCallingConfigMode.
pub fn map_tool_choice(choice: &str) -> &'static str {
    match choice {
        "auto" => "AUTO",
        "none" => "NONE",
        "any" => "ANY",
        _ => "AUTO",
    }
}

/// Map Gemini FinishReason to our StopReason.
pub fn map_stop_reason(reason: &str) -> StopReason {
    match reason {
        "STOP" => StopReason::Stop,
        "MAX_TOKENS" => StopReason::Length,
        _ => StopReason::Error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        AssistantMessage, JsonSchema, KnownApi, KnownProvider, Message, ModelCost, StopReason,
        TextContent, ToolResultMessage, Usage, UserMessage,
    };

    fn test_model() -> Model {
        Model {
            id: "gemini-2.0-flash".to_string(),
            name: String::new(),
            api: crate::types::Api::Known(KnownApi::GoogleGenerativeAi),
            provider: crate::types::Provider::Known(KnownProvider::Google),
            base_url: String::new(),
            reasoning: false,
            thinking_level_map: None,
            input_capabilities: vec![Capability::Text, Capability::Image],
            cost: ModelCost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: 0,
            max_tokens: 0,
            headers: None,
            compat: None,
        }
    }

    #[test]
    fn test_is_thinking_part() {
        assert!(is_thinking_part(Some(true)));
        assert!(!is_thinking_part(Some(false)));
        assert!(!is_thinking_part(None));
    }

    #[test]
    fn test_retain_thought_signature() {
        assert_eq!(
            retain_thought_signature(Some("old".to_string()), Some("new".to_string())),
            Some("new".to_string())
        );
        assert_eq!(
            retain_thought_signature(Some("old".to_string()), None),
            Some("old".to_string())
        );
        assert_eq!(
            retain_thought_signature(Some("old".to_string()), Some(String::new())),
            Some("old".to_string())
        );
    }

    #[test]
    fn test_requires_tool_call_id() {
        assert!(requires_tool_call_id("claude-sonnet-4"));
        assert!(requires_tool_call_id("gpt-oss-4"));
        assert!(!requires_tool_call_id("gemini-2.0-flash"));
    }

    #[test]
    fn test_gemini_version_detection() {
        assert_eq!(get_gemini_major_version("gemini-2.0-flash"), Some(2));
        assert_eq!(get_gemini_major_version("gemini-1.5-pro"), Some(1));
        assert_eq!(get_gemini_major_version("gemini-3.0-pro"), Some(3));
        assert_eq!(get_gemini_major_version("claude-sonnet-4"), None);
    }

    #[test]
    fn test_supports_multimodal() {
        assert!(supports_multimodal_function_response("gemini-3.0-pro"));
        assert!(!supports_multimodal_function_response("gemini-1.5-pro"));
        assert!(supports_multimodal_function_response("claude-sonnet-4"));
    }

    #[test]
    fn test_map_tool_choice() {
        assert_eq!(map_tool_choice("auto"), "AUTO");
        assert_eq!(map_tool_choice("none"), "NONE");
        assert_eq!(map_tool_choice("any"), "ANY");
        assert_eq!(map_tool_choice("unknown"), "AUTO");
    }

    #[test]
    fn test_map_stop_reason() {
        assert_eq!(map_stop_reason("STOP"), StopReason::Stop);
        assert_eq!(map_stop_reason("MAX_TOKENS"), StopReason::Length);
        assert_eq!(map_stop_reason("SAFETY"), StopReason::Error);
    }

    #[test]
    fn test_sanitize_for_openapi() {
        let schema = serde_json::json!({
            "$schema": "http://json-schema.org/...",
            "type": "object",
            "properties": {"name": {"type": "string"}},
            "$defs": {}
        });
        let result = sanitize_for_openapi(&schema);
        assert!(result.get("$schema").is_none());
        assert!(result.get("$defs").is_none());
        assert!(result.get("type").is_some());
        assert!(result.get("properties").is_some());
    }

    #[test]
    fn test_convert_tools_empty() {
        assert!(convert_tools(&[], false).is_none());
    }

    #[test]
    fn test_convert_tools_basic() {
        let tools = vec![ToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters: JsonSchema {
                schema_type: "object".to_string(),
                properties: Some({
                    let mut map = serde_json::Map::new();
                    map.insert("input".to_string(), serde_json::json!({"type": "string"}));
                    map
                }),
                required: None,
                description: None,
                items: None,
                additional_properties: None,
            },
            strict: None,
        }];
        let result = convert_tools(&tools, false);
        assert!(result.is_some());
        let tools = result.unwrap();
        assert_eq!(tools.len(), 1);
        let decls = &tools[0].function_declarations;
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].name, "test_tool");
        assert!(decls[0].parameters_json_schema.is_some());
        assert!(decls[0].parameters.is_none());
    }

    #[test]
    fn test_convert_tools_use_parameters() {
        let tools = vec![ToolDefinition {
            name: "test".to_string(),
            description: "desc".to_string(),
            parameters: JsonSchema {
                schema_type: "object".to_string(),
                properties: None,
                required: None,
                description: None,
                items: None,
                additional_properties: None,
            },
            strict: None,
        }];
        let result = convert_tools(&tools, true);
        let tools = result.unwrap();
        assert!(tools[0].function_declarations[0].parameters.is_some());
        assert!(
            tools[0].function_declarations[0]
                .parameters_json_schema
                .is_none()
        );
    }

    #[test]
    fn test_convert_messages_basic_user() {
        let model = test_model();
        let context = Context {
            system_prompt: None,
            messages: vec![Message::User(UserMessage::text("Hello"))],
            tools: None,
        };
        let contents = convert_messages(&model, &context);
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].role, "user");
        assert_eq!(contents[0].parts[0].text.as_deref(), Some("Hello"));
    }

    #[test]
    fn test_convert_messages_assistant_text() {
        let model = test_model();
        let msg = AssistantMessage::new(
            vec![ContentBlock::Text(TextContent {
                text: "Hi there".to_string(),
                text_signature: None,
            })],
            "google-generative-ai".to_string(),
            "google".to_string(),
            "gemini-2.0-flash".to_string(),
            Usage::zero(),
            StopReason::Stop,
        );
        let context = Context {
            system_prompt: None,
            messages: vec![Message::Assistant(msg)],
            tools: None,
        };
        let contents = convert_messages(&model, &context);
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].role, "model");
        assert_eq!(contents[0].parts[0].text.as_deref(), Some("Hi there"));
    }

    #[test]
    fn test_convert_messages_tool_result() {
        let model = test_model();
        let tool_result = ToolResultMessage::new(
            "call_1",
            "test_tool",
            vec![ContentBlock::Text(TextContent {
                text: "result data".to_string(),
                text_signature: None,
            })],
            false,
        );
        let context = Context {
            system_prompt: None,
            messages: vec![Message::ToolResult(tool_result)],
            tools: None,
        };
        let contents = convert_messages(&model, &context);
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].role, "user");
        assert!(contents[0].parts[0].function_response.is_some());
    }
}
