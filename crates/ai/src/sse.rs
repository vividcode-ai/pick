//! Server-Sent Events (SSE) streaming parser
//! Used by providers to parse `text/event-stream` responses

/// A parsed SSE event
#[derive(Debug, Clone)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

/// Streaming SSE parser that consumes byte chunks and yields complete events
pub struct SseParser {
    buffer: String,
    event: Option<String>,
    data_lines: Vec<String>,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            event: None,
            data_lines: Vec::new(),
        }
    }

    /// Feed a chunk of bytes and yield any complete events
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.buffer.push_str(&String::from_utf8_lossy(chunk));
        self.drain_events()
    }

    /// Feed a text string chunk
    pub fn feed_str(&mut self, text: &str) -> Vec<SseEvent> {
        self.buffer.push_str(text);
        self.drain_events()
    }

    /// Signal end-of-stream and yield any trailing event
    pub fn finish(&mut self) -> Vec<SseEvent> {
        let mut events = self.drain_events();

        // Flush remaining buffered line
        if !self.buffer.is_empty() {
            let line = self.buffer.clone();
            self.buffer.clear();
            self.process_line(&line);
        }

        // Flush pending event
        if let Some(event) = self.flush_event() {
            events.push(event);
        }

        events
    }

    fn drain_events(&mut self) -> Vec<SseEvent> {
        let mut events = Vec::new();

        loop {
            let line = match consume_line(&mut self.buffer) {
                Some(l) => l,
                None => break,
            };

            if line.is_empty() {
                // Blank line = event boundary
                if let Some(event) = self.flush_event() {
                    events.push(event);
                }
            } else if line.starts_with(':') {
                // Comment line, ignore
            } else {
                self.process_line(&line);
            }
        }

        events
    }

    fn process_line(&mut self, line: &str) {
        let delimiter_index = line.find(':');
        let (field, value) = match delimiter_index {
            Some(idx) => {
                let field = &line[..idx];
                let mut value = &line[idx + 1..];
                if value.starts_with(' ') {
                    value = &value[1..];
                }
                (field, value)
            }
            None => (line, ""),
        };

        match field {
            "event" => self.event = Some(value.to_string()),
            "data" => self.data_lines.push(value.to_string()),
            _ => {}
        }
    }

    fn flush_event(&mut self) -> Option<SseEvent> {
        if self.event.is_none() && self.data_lines.is_empty() {
            return None;
        }

        let event = SseEvent {
            event: self.event.take(),
            data: self.data_lines.join("\n"),
        };
        self.data_lines.clear();
        Some(event)
    }
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Try to consume one line from the buffer, returning `None` if incomplete
fn consume_line(buffer: &mut String) -> Option<String> {
    // Find the first line break
    let bytes = buffer.as_bytes();
    let mut found = false;
    let mut line_len = 0;
    let mut skip_next = 0;

    for i in 0..bytes.len() {
        if bytes[i] == b'\n' {
            line_len = i;
            skip_next = 1;
            found = true;
            break;
        }
        if bytes[i] == b'\r' {
            line_len = i;
            // Check for \r\n
            if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                skip_next = 2;
            } else {
                skip_next = 1;
            }
            found = true;
            break;
        }
    }

    if !found {
        return None;
    }

    let line = buffer[..line_len].to_string();
    buffer.drain(..line_len + skip_next);
    Some(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_single_event() {
        let mut parser = SseParser::new();
        let events = parser.feed_str("event: test\ndata: hello world\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event.as_deref(), Some("test"));
        assert_eq!(events[0].data, "hello world");
    }

    #[test]
    fn test_sse_multiple_events() {
        let mut parser = SseParser::new();
        let data = "event: one\ndata: first\n\nevent: two\ndata: second\n\n";
        let events = parser.feed_str(data);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event.as_deref(), Some("one"));
        assert_eq!(events[0].data, "first");
        assert_eq!(events[1].event.as_deref(), Some("two"));
        assert_eq!(events[1].data, "second");
    }

    #[test]
    fn test_sse_crlf() {
        let mut parser = SseParser::new();
        let events = parser.feed_str("event: test\r\ndata: hello\r\n\r\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
    }

    #[test]
    fn test_sse_chunked() {
        let mut parser = SseParser::new();
        let events1 = parser.feed_str("event: test\nda");
        assert_eq!(events1.len(), 0);
        let events2 = parser.feed_str("ta: hello\n\n");
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0].data, "hello");
    }

    #[test]
    fn test_sse_multiline_data() {
        let mut parser = SseParser::new();
        let events = parser.feed_str("event: test\ndata: line1\ndata: line2\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "line1\nline2");
    }

    #[test]
    fn test_sse_trailing_event() {
        let mut parser = SseParser::new();
        let events = parser.feed_str("event: test\ndata: hello\n");
        assert_eq!(events.len(), 0);
        let events = parser.finish();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
    }

    #[test]
    fn test_sse_comment() {
        let mut parser = SseParser::new();
        let events = parser.feed_str(": comment\nevent: test\ndata: hello\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
    }
}
