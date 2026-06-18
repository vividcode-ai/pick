use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use super::truncate::{TruncationResult, TruncationOptions, truncate_tail, DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES};

#[derive(Debug, Clone)]
pub struct OutputSnapshot {
    pub content: String,
    pub truncation: TruncationResult,
    pub full_output_path: Option<String>,
}

/// Incrementally tracks streaming output with bounded memory
pub struct OutputAccumulator {
    max_lines: usize,
    max_bytes: usize,
    max_rolling_bytes: usize,
    temp_file_prefix: String,

    raw_chunks: Vec<Vec<u8>>,
    tail_text: String,
    tail_bytes: usize,
    tail_starts_at_line_boundary: bool,
    total_raw_bytes: usize,
    total_decoded_bytes: usize,
    completed_lines: usize,
    total_lines: usize,
    current_line_bytes: usize,
    has_open_line: bool,
    finished: bool,

    temp_file_path: Option<PathBuf>,
    temp_file: Option<File>,
}

impl OutputAccumulator {
    pub fn new(max_lines: Option<usize>, max_bytes: Option<usize>, temp_file_prefix: Option<&str>) -> Self {
        let max_lines = max_lines.unwrap_or(DEFAULT_MAX_LINES);
        let max_bytes = max_bytes.unwrap_or(DEFAULT_MAX_BYTES);
        Self {
            max_lines,
            max_bytes,
            max_rolling_bytes: (max_bytes * 2).max(1),
            temp_file_prefix: temp_file_prefix.unwrap_or("pick-output").to_string(),
            raw_chunks: Vec::new(),
            tail_text: String::new(),
            tail_bytes: 0,
            tail_starts_at_line_boundary: true,
            total_raw_bytes: 0,
            total_decoded_bytes: 0,
            completed_lines: 0,
            total_lines: 0,
            current_line_bytes: 0,
            has_open_line: false,
            finished: false,
            temp_file_path: None,
            temp_file: None,
        }
    }

    pub fn append(&mut self, data: &[u8]) {
        if self.finished {
            return;
        }
        self.total_raw_bytes += data.len();
        let text = String::from_utf8_lossy(data);
        self.append_decoded_text(&text);

        if self.temp_file.is_some() || self.should_use_temp_file() {
            self.ensure_temp_file();
            if let Some(ref mut f) = self.temp_file {
                let _ = f.write_all(data);
            }
        } else if !data.is_empty() {
            self.raw_chunks.push(data.to_vec());
        }
    }

    pub fn finish(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        if self.should_use_temp_file() {
            self.ensure_temp_file();
        }
    }

    pub fn snapshot(&mut self, persist_if_truncated: bool) -> OutputSnapshot {
        let snapshot_text = self.get_snapshot_text();
        let tail_truncation = truncate_tail(
            &snapshot_text,
            TruncationOptions {
                max_lines: Some(self.max_lines),
                max_bytes: Some(self.max_bytes),
            },
        );

        let truncated = self.total_lines > self.max_lines || self.total_decoded_bytes > self.max_bytes;
        let truncated_by = if truncated {
            Some(
                tail_truncation
                    .truncated_by
                    .clone()
                    .unwrap_or(if self.total_decoded_bytes > self.max_bytes {
                        super::truncate::TruncationType::Bytes
                    } else {
                        super::truncate::TruncationType::Lines
                    }),
            )
        } else {
            None
        };

        let truncation = TruncationResult {
            content: tail_truncation.content.clone(),
            truncated,
            truncated_by,
            total_lines: self.total_lines,
            total_bytes: self.total_decoded_bytes,
            output_lines: tail_truncation.output_lines,
            output_bytes: tail_truncation.output_bytes,
            last_line_partial: tail_truncation.last_line_partial,
            first_line_exceeds_limit: false,
            max_lines: self.max_lines,
            max_bytes: self.max_bytes,
        };

        if persist_if_truncated && truncation.truncated {
            self.ensure_temp_file();
        }

        OutputSnapshot {
            content: truncation.content.clone(),
            truncation,
            full_output_path: self.temp_file_path.as_ref().map(|p| p.to_string_lossy().to_string()),
        }
    }

    pub fn close_temp_file(&mut self) {
        if let Some(mut f) = self.temp_file.take() {
            let _ = f.flush();
        }
    }

    pub fn get_last_line_bytes(&self) -> usize {
        self.current_line_bytes
    }

    fn append_decoded_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let bytes = text.len();
        self.total_decoded_bytes += bytes;
        self.tail_text.push_str(text);
        self.tail_bytes += bytes;
        if self.tail_bytes > self.max_rolling_bytes * 2 {
            self.trim_tail();
        }

        let mut newlines = 0;
        let mut last_newline = None;
        for (i, _) in text.match_indices('\n') {
            newlines += 1;
            last_newline = Some(i);
        }

        if newlines == 0 {
            self.current_line_bytes += bytes;
            self.has_open_line = true;
        } else {
            self.completed_lines += newlines;
            let tail = &text[last_newline.unwrap() + 1..];
            self.current_line_bytes = tail.len();
            self.has_open_line = !tail.is_empty();
        }
        self.total_lines = self.completed_lines + if self.has_open_line { 1 } else { 0 };
    }

    fn trim_tail(&mut self) {
        if self.tail_text.len() <= self.max_rolling_bytes {
            self.tail_bytes = self.tail_text.len();
            return;
        }
        let start = self.tail_text.len() - self.max_rolling_bytes;
        let bytes = self.tail_text.as_bytes();
        let mut adjusted_start = start;
        while adjusted_start < bytes.len() && (bytes[adjusted_start] & 0xc0) == 0x80 {
            adjusted_start += 1;
        }
        self.tail_starts_at_line_boundary = if adjusted_start == 0 {
            self.tail_starts_at_line_boundary
        } else {
            bytes[adjusted_start - 1] == 0x0a
        };
        self.tail_text = String::from_utf8_lossy(&bytes[adjusted_start..]).to_string();
        self.tail_bytes = self.tail_text.len();
    }

    fn get_snapshot_text(&self) -> String {
        if self.tail_starts_at_line_boundary {
            return self.tail_text.clone();
        }
        match self.tail_text.find('\n') {
            Some(pos) => self.tail_text[pos + 1..].to_string(),
            None => self.tail_text.clone(),
        }
    }

    fn should_use_temp_file(&self) -> bool {
        self.total_raw_bytes > self.max_bytes
            || self.total_decoded_bytes > self.max_bytes
            || self.total_lines > self.max_lines
    }

    fn ensure_temp_file(&mut self) {
        if self.temp_file_path.is_some() {
            return;
        }
        let id = uuid::Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext))
            .to_string()[..8]
            .to_string();
        let tmp_path = std::env::temp_dir().join(format!("{}-{}.log", self.temp_file_prefix, id));
        if let Ok(file) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp_path)
        {
            let mut file = file;
            for chunk in &self.raw_chunks {
                let _ = file.write_all(chunk);
            }
            self.temp_file_path = Some(tmp_path);
            self.temp_file = Some(file);
            self.raw_chunks.clear();
        }
    }
}
