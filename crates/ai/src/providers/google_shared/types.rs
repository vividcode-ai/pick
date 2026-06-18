/// Thinking level for Gemini models.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GoogleThinkingLevel {
    Unspecified,
    Minimal,
    Low,
    Medium,
    High,
}

impl GoogleThinkingLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            GoogleThinkingLevel::Unspecified => "THINKING_LEVEL_UNSPECIFIED",
            GoogleThinkingLevel::Minimal => "MINIMAL",
            GoogleThinkingLevel::Low => "LOW",
            GoogleThinkingLevel::Medium => "MEDIUM",
            GoogleThinkingLevel::High => "HIGH",
        }
    }
}
