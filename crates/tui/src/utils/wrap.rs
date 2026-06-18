use crate::utils::ansi::{is_image_line, visible_width};

pub fn wrap_text_with_ansi(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return Vec::new();
    }

    let mut lines: Vec<String> = Vec::new();

    let raw_lines: Vec<&str> = text.lines().collect();
    if raw_lines.iter().any(|l| is_image_line(l)) {
        for raw_line in &raw_lines {
            if is_image_line(raw_line) {
                lines.push(raw_line.to_string());
            } else {
                let wrapped = wrap_words(raw_line, max_width);
                lines.extend(wrapped);
            }
        }
        return lines;
    }

    wrap_words(text, max_width)
}

fn wrap_words(text: &str, max_width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current_line = String::new();
    let mut current_visible: usize = 0;
    let mut has_content = false;

    let mut words: Vec<String> = Vec::new();
    let mut buf = String::new();
    let mut in_escape = false;

    for c in text.chars() {
        if in_escape {
            buf.push(c);
            if c == 'm' {
                in_escape = false;
            }
            continue;
        }
        if c == '\x1b' {
            buf.push(c);
            in_escape = true;
            continue;
        }
        if c == '\n' {
            if !buf.is_empty() {
                words.push(std::mem::take(&mut buf));
            }
            words.push("\n".to_string());
            continue;
        }
        if c == ' ' {
            if !buf.is_empty() {
                words.push(std::mem::take(&mut buf));
            }
            continue;
        }
        buf.push(c);
    }
    if !buf.is_empty() {
        words.push(buf);
    }

    for w in &words {
        if w == "\n" {
            lines.push(std::mem::take(&mut current_line));
            current_visible = 0;
            has_content = false;
            continue;
        }

        let w_visible = visible_width(w);
        if has_content && current_visible + 1 + w_visible > max_width {
            lines.push(std::mem::take(&mut current_line));
            current_visible = 0;
            has_content = false;
        }

        if has_content {
            current_line.push(' ');
            current_visible += 1;
        }

        current_line.push_str(w);
        current_visible += w_visible;
        has_content = true;
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines
}

pub fn apply_background_to_line(
    line: &str,
    width: usize,
    bg_fn: &dyn Fn(&str) -> String,
) -> String {
    let visible_len = visible_width(line);
    let padding_needed = width.saturating_sub(visible_len);
    let padding = " ".repeat(padding_needed);
    let with_padding = format!("{}{}", line, padding);
    bg_fn(&with_padding)
}
