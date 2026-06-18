//! ANSI escape code utilities

/// Strip ANSI escape codes from a string
pub fn strip_ansi(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if let Some('[') = chars.next() {
                for next in &mut chars {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Check if a string contains ANSI escape codes
pub fn has_ansi(text: &str) -> bool {
    text.contains('\x1b')
}
