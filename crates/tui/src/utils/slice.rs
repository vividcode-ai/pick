use unicode_width::UnicodeWidthStr;

use crate::utils::ansi::{AnsiCodeTracker, extract_ansi_code};

pub fn slice_by_column(line: &str, start_col: usize, length: usize) -> String {
    slice_with_width(line, start_col, length).text
}

pub fn slice_with_width(line: &str, start_col: usize, length: usize) -> SliceResult {
    if length == 0 {
        return SliceResult {
            text: String::new(),
            width: 0,
        };
    }

    let end_col = start_col + length;
    let mut result = String::new();
    let mut result_width = 0;
    let mut current_col = 0;
    let mut i = 0;
    let mut pending_ansi = String::new();

    let bytes = line.as_bytes();
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            if let Some(m) = extract_ansi_code(line, i) {
                if current_col >= start_col && current_col < end_col {
                    result.push_str(&m.code);
                } else if current_col < start_col {
                    pending_ansi.push_str(&m.code);
                }
                i += m.length;
                continue;
            }
        }

        let c = line[i..].chars().next().unwrap_or(' ');
        let c_width = UnicodeWidthStr::width(c.to_string().as_str());
        let c_str: String = line[i..]
            .chars()
            .next()
            .map(|c| c.to_string())
            .unwrap_or_default();
        let byte_len = c.len_utf8();

        if current_col >= start_col && current_col < end_col {
            if pending_ansi.ends_with('m') {
                result.push_str(&pending_ansi);
                pending_ansi.clear();
            }
            result.push_str(&c_str);
            result_width += c_width;
        }

        current_col += c_width;
        i += byte_len;

        if current_col >= end_col {
            break;
        }
    }

    SliceResult {
        text: result,
        width: result_width,
    }
}

#[derive(Debug, Clone)]
pub struct SliceResult {
    pub text: String,
    pub width: usize,
}

pub fn extract_segments(
    line: &str,
    before_end: usize,
    after_start: usize,
    after_len: usize,
) -> ExtractSegmentsResult {
    let mut before = String::new();
    let mut before_width = 0;
    let mut after = String::new();
    let mut after_width = 0;
    let mut current_col = 0;
    let mut i = 0;
    let mut pending_ansi_before = String::new();
    let mut after_started = false;
    let after_end = after_start + after_len;

    let mut style_tracker = AnsiCodeTracker::new();

    let bytes = line.as_bytes();
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            if let Some(m) = extract_ansi_code(line, i) {
                style_tracker.process(&m.code);
                if current_col < before_end {
                    pending_ansi_before.push_str(&m.code);
                } else if current_col >= after_start && current_col < after_end && after_started {
                    after.push_str(&m.code);
                }
                i += m.length;
                continue;
            }
        }

        let c = line[i..].chars().next().unwrap_or(' ');
        let c_width = UnicodeWidthStr::width(c.to_string().as_str());
        let c_str: String = line[i..]
            .chars()
            .next()
            .map(|c| c.to_string())
            .unwrap_or_default();
        let byte_len = c.len_utf8();

        if current_col < before_end {
            if !pending_ansi_before.is_empty() {
                before.push_str(&pending_ansi_before);
                pending_ansi_before.clear();
            }
            before.push_str(&c_str);
            before_width += c_width;
        } else if current_col >= after_start && current_col < after_end {
            if !after_started {
                after.push_str(&style_tracker.get_active_codes());
                after_started = true;
            }
            after.push_str(&c_str);
            after_width += c_width;
        }

        current_col += c_width;
        i += byte_len;

        if current_col >= after_end {
            break;
        }
    }

    ExtractSegmentsResult {
        before,
        before_width,
        after,
        after_width,
    }
}

#[derive(Debug, Clone)]
pub struct ExtractSegmentsResult {
    pub before: String,
    pub before_width: usize,
    pub after: String,
    pub after_width: usize,
}
