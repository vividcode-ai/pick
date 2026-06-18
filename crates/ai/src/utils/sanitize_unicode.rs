//! Unicode sanitization utilities

/// Remove or replace invalid/control Unicode characters from a string
pub fn sanitize_unicode(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            // Allow normal printable characters, newlines, and tabs
            if c.is_alphanumeric()
                || c.is_whitespace()
                || c.is_ascii_punctuation()
                || c.is_ascii_graphic()
            {
                return c;
            }
            // Allow common Unicode categories
            if c.is_ascii() && (c as u32) < 32 && c != '\n' && c != '\r' && c != '\t' {
                return '\u{FFFD}'; // replacement character
            }
            // Allow most Unicode but replace surrogates and specials
            let code = c as u32;
            if (0xD800..=0xDFFF).contains(&code) || code == 0xFFFE || code == 0xFFFF {
                return '\u{FFFD}';
            }
            c
        })
        .collect()
}

/// Check if a string contains only safe Unicode characters
pub fn is_safe_unicode(input: &str) -> bool {
    input.chars().all(|c| {
        let code = c as u32;
        c != '\u{FFFD}' && !(0xD800..=0xDFFF).contains(&code) && code != 0xFFFE && code != 0xFFFF
    })
}

/// Remove null bytes from a string
pub fn remove_null_bytes(input: &str) -> String {
    input.chars().filter(|&c| c != '\0').collect()
}
