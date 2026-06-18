//! Session search and filter logic


/// Sort mode for session listing
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortMode {
    Threaded,
    Recent,
    Relevance,
}

/// Name filter for sessions
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NameFilter {
    All,
    Named,
}

/// A parsed search query token
#[derive(Debug, Clone)]
pub enum SearchToken {
    Fuzzy(String),
    Phrase(String),
}

/// Parsed search query
#[derive(Debug, Clone)]
pub struct ParsedSearchQuery {
    pub mode: SearchMode,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum SearchMode {
    Tokens(Vec<SearchToken>),
    Regex(regex::Regex),
    Empty,
}

/// Match result with score
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub matches: bool,
    pub score: f64,
}

/// Session info for search
#[derive(Debug, Clone)]
pub struct SessionSearchInfo {
    pub id: String,
    pub name: Option<String>,
    pub all_messages_text: String,
    pub cwd: String,
    pub modified: String, // ISO8601 timestamp
}

fn normalize_whitespace_lower(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn get_session_search_text(session: &SessionSearchInfo) -> String {
    format!(
        "{} {} {} {}",
        session.id,
        session.name.as_deref().unwrap_or(""),
        session.all_messages_text,
        session.cwd
    )
}

pub fn has_session_name(session: &SessionSearchInfo) -> bool {
    session.name.as_ref().map_or(false, |n| !n.trim().is_empty())
}

fn matches_name_filter(session: &SessionSearchInfo, filter: NameFilter) -> bool {
    match filter {
        NameFilter::All => true,
        NameFilter::Named => has_session_name(session),
    }
}

/// Simple fuzzy match (single token against text)
fn fuzzy_match(token: &str, text: &str) -> MatchResult {
    let lower_token = token.to_lowercase();
    let lower_text = text.to_lowercase();

    // Check if all characters appear in order
    let mut ti = 0;
    let chars: Vec<char> = lower_text.chars().collect();
    for c in lower_token.chars() {
        while ti < chars.len() && chars[ti] != c {
            ti += 1;
        }
        if ti >= chars.len() {
            return MatchResult { matches: false, score: 0.0 };
        }
        ti += 1;
    }

    // Score: fewer characters between matches = better (lower score)
    let score = if lower_text.contains(&lower_token) {
        lower_text.find(&lower_token).unwrap_or(0) as f64 * 0.1
    } else {
        // Approximate score for fuzzy
        text.len() as f64 * 0.5
    };

    MatchResult { matches: true, score }
}

/// Parse search query
pub fn parse_search_query(query: &str) -> ParsedSearchQuery {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return ParsedSearchQuery { mode: SearchMode::Empty, error: None };
    }

    // Regex mode: re:<pattern>
    if let Some(pattern) = trimmed.strip_prefix("re:") {
        let pattern = pattern.trim();
        if pattern.is_empty() {
            return ParsedSearchQuery {
                mode: SearchMode::Empty,
                error: Some("Empty regex".to_string()),
            };
        }
        match regex::Regex::new(&format!("(?i){}", pattern)) {
            Ok(re) => return ParsedSearchQuery { mode: SearchMode::Regex(re), error: None },
            Err(e) => return ParsedSearchQuery { mode: SearchMode::Empty, error: Some(e.to_string()) },
        }
    }

    // Token mode with quote support
    let mut tokens: Vec<SearchToken> = Vec::new();
    let mut buf = String::new();
    let mut in_quote = false;
    let mut had_unclosed_quote = false;

    let flush = |buf: &mut String, kind: &mut Vec<SearchToken>, kind_type: fn(String) -> SearchToken| {
        let v = buf.trim().to_string();
        buf.clear();
        if !v.is_empty() {
            kind.push(kind_type(v));
        }
    };

    for ch in trimmed.chars() {
        if ch == '"' {
            if in_quote {
                flush(&mut buf, &mut tokens, SearchToken::Phrase);
                in_quote = false;
            } else {
                flush(&mut buf, &mut tokens, SearchToken::Fuzzy);
                in_quote = true;
            }
            continue;
        }

        if !in_quote && ch.is_whitespace() {
            let v = buf.trim().to_string();
            if !v.is_empty() {
                tokens.push(SearchToken::Fuzzy(v));
            }
            buf.clear();
            continue;
        }

        buf.push(ch);
    }

    if in_quote {
        had_unclosed_quote = true;
    }

    if had_unclosed_quote {
        // Fall back to whitespace tokenization
        tokens = trimmed
            .split_whitespace()
            .map(|t| SearchToken::Fuzzy(t.to_string()))
            .collect();
    } else {
        let v = buf.trim().to_string();
        if !v.is_empty() {
            tokens.push(if in_quote { SearchToken::Phrase(v) } else { SearchToken::Fuzzy(v) });
        }
    }

    ParsedSearchQuery {
        mode: SearchMode::Tokens(tokens),
        error: None,
    }
}

/// Match a session against a parsed query
pub fn match_session(session: &SessionSearchInfo, parsed: &ParsedSearchQuery) -> MatchResult {
    let text = get_session_search_text(session);

    match &parsed.mode {
        SearchMode::Empty => MatchResult { matches: true, score: 0.0 },
        SearchMode::Regex(re) => {
            match re.find(&text) {
                Some(m) => MatchResult { matches: true, score: m.start() as f64 * 0.1 },
                None => MatchResult { matches: false, score: 0.0 },
            }
        }
        SearchMode::Tokens(tokens) => {
            if tokens.is_empty() {
                return MatchResult { matches: true, score: 0.0 };
            }

            let mut total_score = 0.0;

            for token in tokens {
                match token {
                    SearchToken::Phrase(phrase) => {
                        let normalized_text = normalize_whitespace_lower(&text);
                        let normalized_phrase = normalize_whitespace_lower(phrase);
                        if normalized_phrase.is_empty() {
                            continue;
                        }
                        match normalized_text.find(&normalized_phrase) {
                            Some(idx) => total_score += idx as f64 * 0.1,
                            None => return MatchResult { matches: false, score: 0.0 },
                        }
                    }
                    SearchToken::Fuzzy(fuzzy) => {
                        let m = fuzzy_match(fuzzy, &text);
                        if !m.matches {
                            return MatchResult { matches: false, score: 0.0 };
                        }
                        total_score += m.score;
                    }
                }
            }

            MatchResult { matches: true, score: total_score }
        }
    }
}

/// Filter and sort sessions
pub fn filter_and_sort_sessions(
    sessions: &[SessionSearchInfo],
    query: &str,
    sort_mode: SortMode,
    name_filter: NameFilter,
) -> Vec<SessionSearchInfo> {
    // Apply name filter
    let name_filtered: Vec<_> = sessions
        .iter()
        .filter(|s| matches_name_filter(s, name_filter))
        .cloned()
        .collect();

    let trimmed = query.trim();
    if trimmed.is_empty() {
        return name_filtered;
    }

    let parsed = parse_search_query(query);
    if parsed.error.is_some() {
        return vec![];
    }

    // Recent mode: filter only, keep incoming order
    if sort_mode == SortMode::Recent {
        return name_filtered
            .into_iter()
            .filter(|s| match_session(s, &parsed).matches)
            .collect();
    }

    // Relevance mode: sort by score, tie-break by modified desc
    let mut scored: Vec<(SessionSearchInfo, f64)> = Vec::new();
    for session in name_filtered {
        let res = match_session(&session, &parsed);
        if res.matches {
            scored.push((session, res.score));
        }
    }

    scored.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.0.modified.cmp(&a.0.modified))
    });

    scored.into_iter().map(|(s, _)| s).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(id: &str, name: Option<&str>, text: &str) -> SessionSearchInfo {
        SessionSearchInfo {
            id: id.to_string(),
            name: name.map(|s| s.to_string()),
            all_messages_text: text.to_string(),
            cwd: String::new(),
            modified: String::new(),
        }
    }

    #[test]
    fn test_parse_empty() {
        let q = parse_search_query("");
        assert!(matches!(q.mode, SearchMode::Empty));
    }

    #[test]
    fn test_parse_fuzzy_tokens() {
        let q = parse_search_query("hello world");
        if let SearchMode::Tokens(tokens) = &q.mode {
            assert_eq!(tokens.len(), 2);
        } else {
            panic!("Expected Tokens mode");
        }
    }

    #[test]
    fn test_parse_phrase() {
        let q = parse_search_query(r#""hello world" test"#);
        if let SearchMode::Tokens(tokens) = &q.mode {
            assert_eq!(tokens.len(), 2);
            assert!(matches!(tokens[0], SearchToken::Phrase(_)));
            assert!(matches!(tokens[1], SearchToken::Fuzzy(_)));
        } else {
            panic!("Expected Tokens mode");
        }
    }

    #[test]
    fn test_parse_regex() {
        let q = parse_search_query("re:foo.*bar");
        assert!(matches!(q.mode, SearchMode::Regex(_)));
    }

    #[test]
    fn test_fuzzy_match() {
        let result = fuzzy_match("hlo", "hello world");
        assert!(result.matches);
    }

    #[test]
    fn test_match_session() {
        let session = make_session("test", Some("my session"), "hello world this is content");
        let q = parse_search_query("world");
        let result = match_session(&session, &q);
        assert!(result.matches);
    }
}
