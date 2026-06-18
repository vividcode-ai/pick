//! Faux provider for testing

use crate::types::{
    AssistantMessage, ContentBlock, Context, Model, SimpleStreamOptions, StopReason, StreamEvent, StreamOptions, Usage,
};

/// Faux provider that returns mock responses (for testing)
pub fn stream_faux(
    _model: Model,
    context: Context,
    _options: Option<StreamOptions>,
) -> tokio::sync::mpsc::Receiver<StreamEvent> {
    let (tx, rx) = tokio::sync::mpsc::channel(64);

    tokio::spawn(async move {
        let last_msg = context.messages.last();
        let response_text = if let Some(msg) = last_msg {
            match msg {
                crate::types::Message::User(u) => {
                    u.content.iter()
                        .filter_map(|c| match c {
                            ContentBlock::Text(t) => Some(t.text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                }
                _ => "Hello, I am the faux provider.".to_string(),
            }
        } else {
            "Hello, I am the faux provider.".to_string()
        };

        let msg = AssistantMessage::new(
            vec![ContentBlock::text(response_text)],
            String::new(), "faux".to_string(), "faux-model".to_string(),
            Usage::zero(), StopReason::Stop,
        );

        let _ = tx.send(StreamEvent::Done { reason: StopReason::Stop, message: msg }).await;
    });

    rx
}

/// Faux simple stream
pub fn stream_simple_faux(
    model: Model,
    context: Context,
    _options: Option<SimpleStreamOptions>,
) -> tokio::sync::mpsc::Receiver<StreamEvent> {
    stream_faux(model, context, None)
}
