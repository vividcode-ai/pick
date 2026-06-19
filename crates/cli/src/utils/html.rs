//! HTML entity utilities

/// Decode a single HTML entity at position in string
pub fn decode_html_entity_at(text: &str, index: usize) -> Option<(String, usize)> {
    let remaining = &text[index..];
    if let Some(semi) = remaining.find(';') {
        let entity = &remaining[..=semi];
        let decoded = match entity {
            "&amp;" => "&",
            "&lt;" => "<",
            "&gt;" => ">",
            "&quot;" => "\"",
            "&#39;" => "'",
            "&#x27;" => "'",
            "&#x60;" => "`",
            "&#123;" => "{",
            "&#125;" => "}",
            "&nbsp;" => "\u{00a0}",
            _ => return None,
        };
        Some((decoded.to_string(), entity.len()))
    } else {
        None
    }
}

/// Escape HTML special characters in text
pub fn escape_html(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '&' => result.push_str("&amp;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&#39;"),
            _ => result.push(c),
        }
    }
    result
}

/// Decode all HTML entities in a string
pub fn decode_html_entities(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut i = 0;
    let bytes = text.as_bytes();
    while i < text.len() {
        if bytes[i] == b'&'
            && let Some((decoded, consumed)) = decode_html_entity_at(text, i) {
                result.push_str(&decoded);
                i += consumed;
                continue;
            }
        result.push(text[i..].chars().next().unwrap_or_default());
        i += text[i..].chars().next().map_or(1, |c| c.len_utf8());
    }
    result
}
