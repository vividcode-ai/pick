pub const DEFAULT_MAX_LINES: usize = 2000;
pub const DEFAULT_MAX_BYTES: usize = 50 * 1024; // 50KB
pub const GREP_MAX_LINE_LENGTH: usize = 500;

#[derive(Debug, Clone)]
pub struct TruncationResult {
    pub content: String,
    pub truncated: bool,
    pub truncated_by: Option<TruncationType>,
    pub total_lines: usize,
    pub total_bytes: usize,
    pub output_lines: usize,
    pub output_bytes: usize,
    pub last_line_partial: bool,
    pub first_line_exceeds_limit: bool,
    pub max_lines: usize,
    pub max_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TruncationType {
    Lines,
    Bytes,
}

#[derive(Debug, Clone, Default)]
pub struct TruncationOptions {
    pub max_lines: Option<usize>,
    pub max_bytes: Option<usize>,
}

fn split_lines_for_counting(content: &str) -> Vec<&str> {
    if content.is_empty() {
        return Vec::new();
    }
    let lines: Vec<&str> = content.split('\n').collect();
    if content.ends_with('\n') {
        let mut v = lines;
        v.pop();
        v
    } else {
        lines
    }
}

/// Format bytes as human-readable size
pub fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Truncate from the head (keep first N lines/bytes)
pub fn truncate_head(content: &str, options: TruncationOptions) -> TruncationResult {
    let max_lines = options.max_lines.unwrap_or(DEFAULT_MAX_LINES);
    let max_bytes = options.max_bytes.unwrap_or(DEFAULT_MAX_BYTES);

    let total_bytes = content.len();
    let lines = split_lines_for_counting(content);
    let total_lines = lines.len();

    if total_lines <= max_lines && total_bytes <= max_bytes {
        return TruncationResult {
            content: content.to_string(),
            truncated: false,
            truncated_by: None,
            total_lines,
            total_bytes,
            output_lines: total_lines,
            output_bytes: total_bytes,
            last_line_partial: false,
            first_line_exceeds_limit: false,
            max_lines,
            max_bytes,
        };
    }

    let first_line_bytes = lines.first().map(|l| l.len()).unwrap_or(0);
    if first_line_bytes > max_bytes {
        return TruncationResult {
            content: String::new(),
            truncated: true,
            truncated_by: Some(TruncationType::Bytes),
            total_lines,
            total_bytes,
            output_lines: 0,
            output_bytes: 0,
            last_line_partial: false,
            first_line_exceeds_limit: true,
            max_lines,
            max_bytes,
        };
    }

    let mut output_lines_arr: Vec<&str> = Vec::new();
    let mut output_bytes_count = 0;
    let mut truncated_by = TruncationType::Lines;

    for (i, line) in lines.iter().enumerate() {
        if i >= max_lines {
            break;
        }
        let line_bytes = line.len() + if i > 0 { 1 } else { 0 };

        if output_bytes_count + line_bytes > max_bytes {
            truncated_by = TruncationType::Bytes;
            break;
        }

        output_lines_arr.push(line);
        output_bytes_count += line_bytes;
    }

    if output_lines_arr.len() >= max_lines && output_bytes_count <= max_bytes {
        truncated_by = TruncationType::Lines;
    }

    let output_content = output_lines_arr.join("\n");
    let final_output_bytes = output_content.len();

    TruncationResult {
        content: output_content,
        truncated: true,
        truncated_by: Some(truncated_by),
        total_lines,
        total_bytes,
        output_lines: output_lines_arr.len(),
        output_bytes: final_output_bytes,
        last_line_partial: false,
        first_line_exceeds_limit: false,
        max_lines,
        max_bytes,
    }
}

/// Truncate from the tail (keep last N lines/bytes)
pub fn truncate_tail(content: &str, options: TruncationOptions) -> TruncationResult {
    let max_lines = options.max_lines.unwrap_or(DEFAULT_MAX_LINES);
    let max_bytes = options.max_bytes.unwrap_or(DEFAULT_MAX_BYTES);

    let total_bytes = content.len();
    let lines = split_lines_for_counting(content);
    let total_lines = lines.len();

    if total_lines <= max_lines && total_bytes <= max_bytes {
        return TruncationResult {
            content: content.to_string(),
            truncated: false,
            truncated_by: None,
            total_lines,
            total_bytes,
            output_lines: total_lines,
            output_bytes: total_bytes,
            last_line_partial: false,
            first_line_exceeds_limit: false,
            max_lines,
            max_bytes,
        };
    }

    let mut output_lines_arr: Vec<String> = Vec::new();
    let mut output_bytes_count = 0;
    let mut truncated_by = TruncationType::Lines;
    let mut last_line_partial = false;

    for i in (0..lines.len()).rev() {
        if output_lines_arr.len() >= max_lines {
            break;
        }
        let line = lines[i];
        let line_bytes = line.len() + if output_lines_arr.is_empty() { 0 } else { 1 };

        if output_bytes_count + line_bytes > max_bytes {
            truncated_by = TruncationType::Bytes;
            if output_lines_arr.is_empty() {
                let truncated_line = truncate_string_to_bytes_from_end(line, max_bytes);
                output_lines_arr.insert(0, truncated_line);
                output_bytes_count = output_lines_arr[0].len();
                last_line_partial = true;
            }
            break;
        }

        output_lines_arr.insert(0, line.to_string());
        output_bytes_count += line_bytes;
    }

    if output_lines_arr.len() >= max_lines && output_bytes_count <= max_bytes {
        truncated_by = TruncationType::Lines;
    }

    let output_content = output_lines_arr.join("\n");
    let final_output_bytes = output_content.len();

    TruncationResult {
        content: output_content,
        truncated: true,
        truncated_by: Some(truncated_by),
        total_lines,
        total_bytes,
        output_lines: output_lines_arr.len(),
        output_bytes: final_output_bytes,
        last_line_partial,
        first_line_exceeds_limit: false,
        max_lines,
        max_bytes,
    }
}

fn truncate_string_to_bytes_from_end(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let start = bytes.len() - max_bytes;
    // Find valid UTF-8 boundary
    let mut adjusted_start = start;
    while adjusted_start < bytes.len() && (bytes[adjusted_start] & 0xc0) == 0x80 {
        adjusted_start += 1;
    }
    String::from_utf8_lossy(&bytes[adjusted_start..]).to_string()
}

/// Truncate a single line to max characters
pub fn truncate_line(line: &str, max_chars: usize) -> (String, bool) {
    if line.len() <= max_chars {
        return (line.to_string(), false);
    }
    let truncated: String = line.chars().take(max_chars).collect();
    (format!("{}... [truncated]", truncated), true)
}
