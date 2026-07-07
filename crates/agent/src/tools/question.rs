//! Question tool - asks the user for input

use pick_ai::types::ContentBlock;

use crate::core::state::{
    AgentTool, AgentToolResult, QuestionPrompt, ToolContext, ToolExecutionMode,
};

/// Create the question tool
pub fn create_question_tool() -> AgentTool {
    let params = pick_ai::types::JsonSchema {
        schema_type: "object".to_string(),
        properties: Some(
            vec![(
                "questions".to_string(),
                serde_json::json!({
                    "type": "array",
                    "description": "Questions to ask the user",
                    "items": {
                        "type": "object",
                        "properties": {
                            "question": { "type": "string", "description": "Complete question to ask" },
                            "header": { "type": "string", "description": "Very short label (max 30 chars)" },
                            "options": {
                                "type": "array",
                                "description": "Available choices for the user",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "label": { "type": "string", "description": "Display text (1-5 words)" },
                                        "description": { "type": "string", "description": "Explanation of the choice" }
                                    },
                                    "required": ["label", "description"]
                                }
                            },
                            "multiple": {
                                "type": "boolean",
                                "description": "Allow selecting multiple choices"
                            }
                        },
                        "required": ["question", "header", "options"]
                    }
                }),
            )]
            .into_iter()
            .collect(),
        ),
        required: Some(vec!["questions".to_string()]),
        description: Some(
            "Use this tool when you need to ask the user questions during execution. \
             This allows you to gather user preferences, clarify ambiguous instructions, \
             or get decisions on implementation choices."
            .to_string(),
        ),
        items: None,
        additional_properties: Some(false),
    };

    AgentTool {
        name: "question".to_string(),
        description: "Ask the user questions and wait for their response.".to_string(),
        prompt_snippet: Some("Ask the user questions with question tool".to_string()),
        prompt_guidelines: vec![],
        usage_example: None,
        label: "question".to_string(),
        parameters: params,
        execute: std::sync::Arc::new(move |_tool_call_id, args, ctx: ToolContext| {
            Box::pin(async move {
                let questions_val = args
                    .get("questions")
                    .ok_or_else(|| "Missing 'questions' argument".to_string())?;

                let prompts: Vec<QuestionPrompt> = serde_json::from_value(questions_val.clone())
                    .map_err(|e| format!("Invalid questions format: {}", e))?;

                if prompts.is_empty() {
                    return Ok(AgentToolResult {
                        content: vec![ContentBlock::text("Error: No questions provided")],
                        is_error: true,
                        terminate: false,
                    });
                }

                // Notify observers that we are about to wait for user input
                if let Some(ref bus) = ctx.tool_event_bus
                    && let Some(first) = prompts.first()
                {
                    let kind = crate::core::hooks::WaitingKind::Question {
                        header: first.header.clone(),
                        question: first.question.clone(),
                    };
                    bus.publish(&crate::core::hooks::ToolEvent::WaitingForUser {
                        tool_name: "question".to_string(),
                        tool_call_id: _tool_call_id.to_string(),
                        input: questions_val.clone(),
                        kind,
                        summary: format!("[{}] {}", first.header, first.question),
                    })
                    .await;
                }

                let answers = match ctx.question {
                    Some(ref q_fn) => q_fn(prompts.clone())
                        .await
                        .map_err(|e| format!("Question error: {}", e))?,
                    None => {
                        return Ok(AgentToolResult {
                            content: vec![ContentBlock::text(
                                "Error: question tool is not available in this mode (no interactive input)",
                            )],
                            is_error: true,
                            terminate: false,
                        });
                    }
                };

                let formatted = prompts
                    .iter()
                    .zip(answers.iter())
                    .map(|(q, a)| format!("\"{}\"=\"{}\"", q.question, a.join(", ")))
                    .collect::<Vec<_>>()
                    .join(", ");

                Ok(AgentToolResult {
                    content: vec![ContentBlock::text(format!(
                        "User has answered your questions: {}. You can now continue with the user's answers in mind.",
                        formatted
                    ))],
                    is_error: false,
                    terminate: false,
                })
            })
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}
