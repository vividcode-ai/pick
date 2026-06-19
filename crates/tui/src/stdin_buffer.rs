//! Stdin buffering for terminal input

/// Status of an escape sequence
#[derive(Debug, Clone, Copy, PartialEq)]
enum SequenceStatus {
    Complete,
    Incomplete,
    NotEscape,
}

const ESC: char = '\x1b';
const BRACKETED_PASTE_START: &str = "\x1b[200~";
const BRACKETED_PASTE_END: &str = "\x1b[201~";

fn is_complete_sequence(data: &str) -> SequenceStatus {
    if !data.starts_with(ESC) {
        return SequenceStatus::NotEscape;
    }

    if data.len() == 1 {
        return SequenceStatus::Incomplete;
    }

    let after_esc = &data[1..];

    // CSI sequences: ESC [
    if after_esc.starts_with('[') {
        // Check for old-style mouse: ESC[M + 3 bytes
        if after_esc.starts_with("[M") {
            return if data.len() >= 6 {
                SequenceStatus::Complete
            } else {
                SequenceStatus::Incomplete
            };
        }
        return is_complete_csi(data);
    }

    // SS3: ESC O
    if after_esc.starts_with('O') {
        return if after_esc.len() >= 2 {
            SequenceStatus::Complete
        } else {
            SequenceStatus::Incomplete
        };
    }

    // Meta key: ESC + single char
    if after_esc.len() == 1 {
        return SequenceStatus::Complete;
    }

    // OSC: ESC ]
    if after_esc.starts_with(']') {
        return is_complete_osc(data);
    }

    // DCS: ESC P
    if after_esc.starts_with('P') {
        return is_complete_dcs(data);
    }

    // APC: ESC _
    if after_esc.starts_with('_') {
        return is_complete_apc(data);
    }

    SequenceStatus::Complete
}

fn is_complete_csi(data: &str) -> SequenceStatus {
    if !data.starts_with(&format!("{}[", ESC)) {
        return SequenceStatus::Complete;
    }
    if data.len() < 3 {
        return SequenceStatus::Incomplete;
    }

    let payload = &data[2..];
    let last = payload.chars().last().unwrap();
    let last_code = last as u32;

    // CSI ends with byte 0x40-0x7E (@-~)
    if (0x40..=0x7e).contains(&last_code) {
        // Special: SGR mouse ESC[<B;X;Ym
        if payload.starts_with('<') {
            let mouse_match = regex::Regex::new(r"^<\d+;\d+;\d+[Mm]$")
                .unwrap()
                .is_match(payload);
            if mouse_match {
                return SequenceStatus::Complete;
            }
            if last == 'M' || last == 'm' {
                let parts: Vec<&str> = payload[1..payload.len() - 1].split(';').collect();
                if parts.len() == 3 && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
                    return SequenceStatus::Complete;
                }
            }
            return SequenceStatus::Incomplete;
        }
        return SequenceStatus::Complete;
    }

    SequenceStatus::Incomplete
}

fn is_complete_osc(data: &str) -> SequenceStatus {
    if !data.starts_with(&format!("{}]", ESC)) {
        return SequenceStatus::Complete;
    }
    if data.ends_with(&format!("{}\\", ESC)) || data.ends_with('\x07') {
        return SequenceStatus::Complete;
    }
    SequenceStatus::Incomplete
}

fn is_complete_dcs(data: &str) -> SequenceStatus {
    if !data.starts_with(&format!("{}P", ESC)) || !data.ends_with(&format!("{}\\", ESC)) {
        return SequenceStatus::Incomplete;
    }
    SequenceStatus::Complete
}

fn is_complete_apc(data: &str) -> SequenceStatus {
    if !data.starts_with(&format!("{}_", ESC)) || !data.ends_with(&format!("{}\\", ESC)) {
        return SequenceStatus::Incomplete;
    }
    SequenceStatus::Complete
}

fn extract_complete_sequences(buffer: &str) -> (Vec<String>, String) {
    let mut sequences = Vec::new();
    let mut pos = 0;
    let chars: Vec<char> = buffer.chars().collect();

    while pos < chars.len() {
        if chars[pos] == ESC {
            let remaining: String = chars[pos..].iter().collect();
            let mut seq_end = 1;

            while seq_end <= remaining.len() {
                let candidate: String = remaining.chars().take(seq_end).collect();
                match is_complete_sequence(&candidate) {
                    SequenceStatus::Complete => {
                        if candidate == "\x1b\x1b" {
                            let next = remaining.chars().nth(seq_end);
                            if let Some(nc) = next
                                && (nc == '[' || nc == ']' || nc == 'O' || nc == 'P' || nc == '_')
                            {
                                sequences.push(ESC.to_string());
                                pos += 1;
                                break;
                            }
                        }
                        sequences.push(candidate);
                        pos += seq_end;
                        break;
                    }
                    SequenceStatus::Incomplete => {
                        seq_end += 1;
                    }
                    SequenceStatus::NotEscape => {
                        sequences.push(candidate);
                        pos += seq_end;
                        break;
                    }
                }
            }

            if seq_end > remaining.len() {
                return (sequences, remaining);
            }
        } else {
            sequences.push(chars[pos].to_string());
            pos += 1;
        }
    }

    (sequences, String::new())
}

/// Buffers stdin input and emits complete sequences.
/// Handles partial escape sequences that arrive across multiple chunks.
pub struct StdinBuffer {
    buffer: String,
    paste_mode: bool,
    paste_buffer: String,
    pending_kitty_codepoint: Option<u32>,
}

impl StdinBuffer {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            paste_mode: false,
            paste_buffer: String::new(),
            pending_kitty_codepoint: None,
        }
    }

    /// Process incoming data and return complete sequences and paste events.
    /// Returns (sequences, paste_content, remainder)
    pub fn process(&mut self, data: &str) -> (Vec<String>, Option<String>) {
        // Handle high-byte conversion
        let str_data = if data.len() == 1 && data.as_bytes()[0] > 127 {
            let byte = data.as_bytes()[0] - 128;
            format!("\x1b{}", byte as char)
        } else {
            data.to_string()
        };

        if str_data.is_empty() && self.buffer.is_empty() {
            return (vec![String::new()], None);
        }

        self.buffer.push_str(&str_data);

        if self.paste_mode {
            self.paste_buffer.push_str(&self.buffer);
            self.buffer.clear();

            if let Some(end_idx) = self.paste_buffer.find(BRACKETED_PASTE_END) {
                let pasted = self.paste_buffer[..end_idx].to_string();
                let remaining =
                    self.paste_buffer[end_idx + BRACKETED_PASTE_END.len()..].to_string();

                self.paste_mode = false;
                self.paste_buffer.clear();
                self.pending_kitty_codepoint = None;

                let mut sequences = Vec::new();
                if !remaining.is_empty() {
                    let (rest_seqs, _) = self.process(&remaining);
                    sequences = rest_seqs;
                }
                return (sequences, Some(pasted));
            }
            return (Vec::new(), None);
        }

        // Check for bracketed paste start
        if let Some(start_idx) = self.buffer.find(BRACKETED_PASTE_START) {
            if start_idx > 0 {
                let before = &self.buffer[..start_idx];
                let (seqs, _) = extract_complete_sequences(before);
                for s in &seqs {
                    self.emit_data_sequence(s);
                }
            }

            self.pending_kitty_codepoint = None;
            self.buffer = self.buffer[start_idx + BRACKETED_PASTE_START.len()..].to_string();
            self.paste_mode = true;
            self.paste_buffer = self.buffer.clone();
            self.buffer.clear();

            if let Some(end_idx) = self.paste_buffer.find(BRACKETED_PASTE_END) {
                let pasted = self.paste_buffer[..end_idx].to_string();
                let remaining =
                    self.paste_buffer[end_idx + BRACKETED_PASTE_END.len()..].to_string();

                self.paste_mode = false;
                self.paste_buffer.clear();
                self.pending_kitty_codepoint = None;

                let mut sequences = Vec::new();
                if !remaining.is_empty() {
                    let (rest_seqs, _) = self.process(&remaining);
                    sequences = rest_seqs;
                }
                return (sequences, Some(pasted));
            }
            return (Vec::new(), None);
        }

        let (sequences, remainder) = extract_complete_sequences(&self.buffer);
        self.buffer = remainder;
        (sequences, None)
    }

    fn emit_data_sequence(&mut self, sequence: &str) {
        if sequence.len() == 1
            && let Some(cp) = sequence.chars().next().map(|c| c as u32)
            && Some(cp) == self.pending_kitty_codepoint
        {
            self.pending_kitty_codepoint = None;
            return;
        }
        self.pending_kitty_codepoint = parse_kitty_printable_codepoint(sequence);
    }

    /// Flush remaining buffer and return all pending sequences.
    pub fn flush(&mut self) -> Vec<String> {
        if self.buffer.is_empty() {
            return Vec::new();
        }
        let sequences = vec![self.buffer.clone()];
        self.buffer.clear();
        self.pending_kitty_codepoint = None;
        sequences
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.paste_mode = false;
        self.paste_buffer.clear();
        self.pending_kitty_codepoint = None;
    }
}

impl Default for StdinBuffer {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_kitty_printable_codepoint(sequence: &str) -> Option<u32> {
    let re = regex::Regex::new(r"^\x1b\[(\d+)(?::\d*)?(?::\d+)?u$").ok()?;
    let caps = re.captures(sequence)?;
    let codepoint: u32 = caps.get(1)?.as_str().parse().ok()?;
    if codepoint >= 32 {
        Some(codepoint)
    } else {
        None
    }
}
