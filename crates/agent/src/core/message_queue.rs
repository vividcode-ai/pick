//! Pending message queue — a queuing primitive for steering/follow-up/next-turn messages.
//!
//! Supports two drain modes:
//! - `All` — drain all queued messages at once
//! - `OneAtATime` — drain only the oldest message per call

use std::collections::VecDeque;

use pick_ai::types::Message;

/// Controls whether all messages are drained at once or one at a time
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QueueMode {
    /// Drain all queued messages at once
    All,
    /// Drain only the oldest message per drain() call
    OneAtATime,
}

impl QueueMode {
    pub fn is_all(&self) -> bool {
        matches!(self, QueueMode::All)
    }

    pub fn is_one_at_a_time(&self) -> bool {
        matches!(self, QueueMode::OneAtATime)
    }
}

/// A pending message queue with configurable drain mode.
///
/// This is the primitive used for steering, follow-up, and next-turn queues:
/// - **Steering**: injected into the running agent between turns
/// - **Follow-up**: injected after the agent would naturally stop
/// - **Next-turn**: survives abort and is prepended to the next user prompt
#[derive(Debug)]
pub struct PendingMessageQueue {
    messages: VecDeque<Message>,
    mode: QueueMode,
}

impl PendingMessageQueue {
    /// Create a new queue with the given drain mode
    pub fn new(mode: QueueMode) -> Self {
        Self {
            messages: VecDeque::new(),
            mode,
        }
    }

    /// Enqueue a message at the back of the queue
    pub fn enqueue(&mut self, message: Message) {
        self.messages.push_back(message);
    }

    /// Drain messages according to the current QueueMode:
    /// - `All`: returns all messages and clears the queue
    /// - `OneAtATime`: returns the oldest message (front) only
    pub fn drain(&mut self) -> Vec<Message> {
        match self.mode {
            QueueMode::All => self.messages.drain(..).collect(),
            QueueMode::OneAtATime => {
                if let Some(msg) = self.messages.pop_front() {
                    vec![msg]
                } else {
                    vec![]
                }
            }
        }
    }

    /// Check if the queue has any items
    pub fn has_items(&self) -> bool {
        !self.messages.is_empty()
    }

    /// Clear all messages from the queue
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Return the number of queued messages
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Return true if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Get the current drain mode
    pub fn mode(&self) -> QueueMode {
        self.mode
    }

    /// Set the drain mode
    pub fn set_mode(&mut self, mode: QueueMode) {
        self.mode = mode;
    }

    /// Peek at the front message without draining
    pub fn peek(&self) -> Option<&Message> {
        self.messages.front()
    }

    /// Drain all messages regardless of QueueMode
    pub fn drain_all(&mut self) -> Vec<Message> {
        self.messages.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pick_ai::types::UserMessage;

    fn user_msg(text: &str) -> Message {
        Message::User(UserMessage::text(text))
    }

    #[test]
    fn test_one_at_a_time() {
        use pick_ai::types::Message;

        let mut q = PendingMessageQueue::new(QueueMode::OneAtATime);
        q.enqueue(user_msg("a"));
        q.enqueue(user_msg("b"));
        q.enqueue(user_msg("c"));

        assert_eq!(q.len(), 3);
        assert!(q.has_items());

        let drained = q.drain();
        assert_eq!(drained.len(), 1);
        assert!(matches!(drained[0], Message::User(_)));
        assert_eq!(q.len(), 2);

        let drained = q.drain();
        assert_eq!(drained.len(), 1);
        assert!(matches!(drained[0], Message::User(_)));
        assert_eq!(q.len(), 1);

        let drained = q.drain();
        assert_eq!(drained.len(), 1);
        assert!(matches!(drained[0], Message::User(_)));
        assert!(!q.has_items());

        let drained = q.drain();
        assert!(drained.is_empty());
    }

    #[test]
    fn test_all_mode() {
        let mut q = PendingMessageQueue::new(QueueMode::All);
        q.enqueue(user_msg("a"));
        q.enqueue(user_msg("b"));
        q.enqueue(user_msg("c"));

        let drained = q.drain();
        assert_eq!(drained.len(), 3);
        assert!(drained.iter().all(|m| matches!(m, Message::User(_))));
        assert!(q.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut q = PendingMessageQueue::new(QueueMode::OneAtATime);
        q.enqueue(user_msg("a"));
        q.enqueue(user_msg("b"));
        q.clear();
        assert!(q.is_empty());
        assert_eq!(q.drain().len(), 0);
    }

    #[test]
    fn test_drain_all_ignores_mode() {
        let mut q = PendingMessageQueue::new(QueueMode::OneAtATime);
        q.enqueue(user_msg("a"));
        q.enqueue(user_msg("b"));

        let drained = q.drain_all();
        assert_eq!(drained.len(), 2);
        assert!(q.is_empty());
    }

    #[test]
    fn test_set_mode() {
        let mut q = PendingMessageQueue::new(QueueMode::OneAtATime);
        assert!(q.mode().is_one_at_a_time());
        q.set_mode(QueueMode::All);
        assert!(q.mode().is_all());
    }

    #[test]
    fn test_peek() {
        use pick_ai::types::Message;

        let mut q = PendingMessageQueue::new(QueueMode::OneAtATime);
        assert!(q.peek().is_none());
        q.enqueue(user_msg("a"));
        q.enqueue(user_msg("b"));
        assert!(matches!(q.peek(), Some(Message::User(_))));
        q.drain();
        assert!(matches!(q.peek(), Some(Message::User(_))));
    }
}
