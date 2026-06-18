//! Generic event stream with push-based async iteration

/// A generic event stream that supports push-based event delivery.
/// Events can be pushed in and consumed via next_event().
/// Automatically detects completion and extracts a final result.
pub struct EventStream<T, R = T> {
    queue: Vec<T>,
    done: bool,
    final_result: Option<R>,
    _extract_result: Box<dyn Fn(&T) -> R + Send>,
}

impl<T, R> EventStream<T, R> {
    pub fn new(extract_result: Box<dyn Fn(&T) -> R + Send>) -> Self {
        Self {
            queue: Vec::new(),
            done: false,
            final_result: None,
            _extract_result: extract_result,
        }
    }

    /// Push an event into the stream. Ignored after completion.
    pub fn push(&mut self, event: T) {
        if self.done {
            return;
        }
        self.queue.push(event);
    }

    /// Mark the stream as complete.
    pub fn end(&mut self, result: Option<R>) {
        self.done = true;
        if let Some(r) = result {
            self.final_result = Some(r);
        }
    }

    /// Get the next event, or None if queue is empty.
    pub fn next_event(&mut self) -> Option<T> {
        if !self.queue.is_empty() {
            return Some(self.queue.remove(0));
        }
        None
    }

    /// Check if the stream is done and queue is empty.
    pub fn is_done(&self) -> bool {
        self.done && self.queue.is_empty()
    }

    /// Get the final result, if available.
    pub fn result(&self) -> Option<&R> {
        self.final_result.as_ref()
    }
}

/// Create an EventStream specialized for assistant message events.
/// Uses the existing AssistantMessageEventStream from the types module.
pub fn create_assistant_message_event_stream() -> crate::types::AssistantMessageEventStream {
    let (_tx, rx) = tokio::sync::mpsc::channel(64);
    crate::types::AssistantMessageEventStream::new(rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_stream_push() {
        let mut stream = EventStream::new(Box::new(|v: &i32| *v));
        stream.push(1);
        stream.push(2);
        assert_eq!(stream.next_event(), Some(1));
        assert_eq!(stream.next_event(), Some(2));
        assert_eq!(stream.next_event(), None);
    }

    #[test]
    fn test_event_stream_done() {
        let mut stream = EventStream::new(Box::new(|v: &i32| *v));
        stream.end(Some(42));
        assert!(stream.is_done());
        assert_eq!(stream.result(), Some(&42));
    }

    #[test]
    fn test_event_stream_no_more_after_done() {
        let mut stream = EventStream::new(Box::new(|v: &i32| *v));
        stream.end(Some(99));
        stream.push(1);
        assert_eq!(stream.next_event(), None);
        assert!(stream.is_done());
    }
}
