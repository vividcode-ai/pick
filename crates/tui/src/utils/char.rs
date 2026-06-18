pub fn is_whitespace_char(c: char) -> bool {
    c.is_whitespace()
}

pub fn is_punctuation_char(c: char) -> bool {
    matches!(c, '(' | ')' | '{' | '}' | '[' | ']' | '<' | '>' | '.' | ',' | ';' | ':' | '\'' | '"'
        | '!' | '?' | '+' | '-' | '=' | '*' | '/' | '\\' | '|' | '&' | '%' | '^' | '$' | '#' | '@' | '~' | '`')
}
