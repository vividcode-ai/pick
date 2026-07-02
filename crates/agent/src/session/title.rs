//! Shared session title auto-generation.
//! Can be used by both TUI and web server.

use pick_ai::types::Message;

/// System prompt for the title-generation LLM call.
/// Instructs the model to produce a brief, single-line, language-matched title.
pub const TITLE_PROMPT: &str = "\
Generate a brief title that would help the user find this conversation later.

Your output must be:
- A single line
- No more than 50 characters
- No explanations, no quotes, no extra text

Rules:
- You MUST use the same language as the user message you are summarizing
- Title must be grammatically correct and read naturally - no word salad
- Never include tool names in the title
- Focus on the main topic or question the user needs to retrieve
- Vary your phrasing
- Keep exact: technical terms, numbers, filenames, HTTP codes
- Remove: the, this, my, a, an
- Never assume tech stack
- Never respond to questions, just generate a title for the conversation
- Always output something meaningful, even if the input is minimal
- If the user message is short or conversational, create a title that reflects the tone";

fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &text[..end])
}

/// Clean the raw LLM response into a usable title.
/// Takes the first non-empty, non-tag line, strips surrounding quotes, and truncates to 100 chars.
pub fn clean_title(text: &str) -> Option<String> {
    text.lines()
        .filter_map(|l| {
            let trimmed = l.trim();
            if trimmed.is_empty() || trimmed.starts_with('<') {
                None
            } else {
                Some(
                    trimmed
                        .trim_matches('"')
                        .trim_matches('\'')
                        .trim()
                        .to_string(),
                )
            }
        })
        .next()
        .filter(|l| !l.is_empty())
        .map(|l| if l.len() > 100 { truncate(&l, 97) } else { l })
}

/// Extract the text content from the first user message.
pub fn first_user_text(messages: &[Message]) -> Option<String> {
    messages.iter().find_map(|m| match m {
        Message::User(u) => {
            let text: String = u
                .content
                .iter()
                .filter_map(|b| {
                    if let pick_ai::types::content::ContentBlock::Text(t) = b {
                        Some(t.text.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            if text.is_empty() { None } else { Some(text) }
        }
        _ => None,
    })
}

/// Generate a title from user message text using the configured model.
///
/// Tries twice:
/// 1. With the full system prompt
/// 2. With a simpler fallback prompt (if the first attempt returns nothing)
///
/// Returns a fallback (truncated raw text) if both attempts fail.
pub async fn generate_title(
    title_text: &str,
    model: &pick_ai::types::Model,
    api_key: Option<String>,
) -> Option<String> {
    async fn call_provider(
        mdl: &pick_ai::types::Model,
        api_key: Option<String>,
        ctx: pick_ai::Context,
    ) -> Option<String> {
        let result = pick_ai::complete_simple(mdl, ctx, api_key, None, Some(100), None, None).await;
        if result.error_message.is_some() {
            return None;
        }
        let text: String = result
            .content
            .iter()
            .filter_map(|b| {
                if let pick_ai::types::content::ContentBlock::Text(t) = b {
                    Some(t.text.as_str())
                } else {
                    None
                }
            })
            .collect();
        if text.is_empty() { None } else { Some(text) }
    }

    let ctx1 = pick_ai::Context {
        system_prompt: Some(TITLE_PROMPT.to_string()),
        messages: vec![Message::User(pick_ai::UserMessage::text(format!(
            "Generate a title in the SAME LANGUAGE as the user message below. \
             Only output the title, nothing else.\n\n{}",
            title_text
        )))],
        tools: None,
    };
    let resp1 = call_provider(model, api_key.clone(), ctx1).await;
    let mut title = resp1.as_deref().and_then(clean_title);

    if title.is_none() {
        let ctx2 = pick_ai::Context {
            system_prompt: None,
            messages: vec![Message::User(pick_ai::UserMessage::text(format!(
                "Generate a very short title in the SAME LANGUAGE as the user message. \
                 Max 40 characters, no quotes, no explanation.\n\n{}",
                title_text
            )))],
            tools: None,
        };
        let resp2 = call_provider(model, api_key.clone(), ctx2).await;
        title = resp2.as_deref().and_then(clean_title);
    }

    Some(title.unwrap_or_else(|| {
        if title_text.len() > 50 {
            truncate(title_text, 47)
        } else {
            title_text.to_string()
        }
    }))
}
