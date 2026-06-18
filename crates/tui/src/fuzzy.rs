//! Fuzzy matching utilities

/// Result of a fuzzy match
#[derive(Debug, Clone, Copy)]
pub struct FuzzyMatch {
    pub matches: bool,
    pub score: f64,
}

/// Fuzzy match a query against text.
/// All query characters must appear in order. Lower score = better match.
pub fn fuzzy_match(query: &str, text: &str) -> FuzzyMatch {
    let query_lower = query.to_lowercase();
    let text_lower = text.to_lowercase();

    let primary = match_query(&query_lower, &text_lower);
    if primary.matches {
        return primary;
    }

    // Try alphanumeric swap (e.g., "abc123" -> "123abc")
    let swapped = try_swap_query(&query_lower);
    match swapped {
        Some(swapped) => {
            let swapped_match = match_query(&swapped, &text_lower);
            if swapped_match.matches {
                return FuzzyMatch {
                    matches: true,
                    score: swapped_match.score + 5.0,
                };
            }
            primary
        }
        None => primary,
    }
}

fn match_query(query: &str, text: &str) -> FuzzyMatch {
    if query.is_empty() {
        return FuzzyMatch {
            matches: true,
            score: 0.0,
        };
    }
    if query.len() > text.len() {
        return FuzzyMatch {
            matches: false,
            score: 0.0,
        };
    }

    let mut query_idx = 0;
    let mut score = 0.0_f64;
    let mut last_match: i32 = -1;
    let mut consecutive = 0;
    let query_chars: Vec<char> = query.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();

    for (i, &tc) in text_chars.iter().enumerate() {
        if query_idx >= query_chars.len() {
            break;
        }
        if tc == query_chars[query_idx] {
            let is_word_boundary = i == 0
                || text_chars.get(i.wrapping_sub(1)).map_or(false, |&c| {
                    c == ' ' || c == '-' || c == '_' || c == '.' || c == '/' || c == ':'
                });

            if last_match >= 0 && i as i32 == last_match + 1 {
                consecutive += 1;
                score -= (consecutive as f64) * 5.0;
            } else {
                consecutive = 0;
                if last_match >= 0 {
                    score += ((i as i32) - last_match - 1) as f64 * 2.0;
                }
            }

            if is_word_boundary {
                score -= 10.0;
            }

            score += (i as f64) * 0.1;

            last_match = i as i32;
            query_idx += 1;
        }
    }

    if query_idx < query_chars.len() {
        return FuzzyMatch {
            matches: false,
            score: 0.0,
        };
    }

    if query == text {
        score -= 100.0;
    }

    FuzzyMatch {
        matches: true,
        score,
    }
}

fn try_swap_query(query: &str) -> Option<String> {
    let letters: String = query.chars().filter(|c| c.is_ascii_alphabetic()).collect();
    let digits: String = query.chars().filter(|c| c.is_ascii_digit()).collect();

    if letters.is_empty() || digits.is_empty() {
        return None;
    }

    // Check pattern: letters then digits
    let mut after_letters = false;
    let mut has_letters_then_digits = true;
    for c in query.chars() {
        if c.is_ascii_alphabetic() {
            after_letters = true;
        } else if c.is_ascii_digit() && after_letters {
            // valid
        } else {
            has_letters_then_digits = false;
        }
    }

    if has_letters_then_digits {
        return Some(format!("{}{}", digits, letters));
    }

    // Check pattern: digits then letters
    let mut after_digits = false;
    let mut has_digits_then_letters = true;
    for c in query.chars() {
        if c.is_ascii_digit() {
            after_digits = true;
        } else if c.is_ascii_alphabetic() && after_digits {
            // valid
        } else {
            has_digits_then_letters = false;
        }
    }

    if has_digits_then_letters {
        return Some(format!("{}{}", letters, digits));
    }

    None
}

/// Filter and sort items by fuzzy match quality (best matches first).
/// Supports space-separated tokens: all tokens must match.
pub fn fuzzy_filter<T>(items: &[T], query: &str, get_text: impl Fn(&T) -> &str) -> Vec<T>
where
    T: Clone,
{
    if query.trim().is_empty() {
        return items.to_vec();
    }

    let tokens: Vec<&str> = query
        .trim()
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .collect();

    if tokens.is_empty() {
        return items.to_vec();
    }

    let mut results: Vec<(T, f64)> = Vec::new();

    'items: for item in items {
        let text = get_text(item);
        let mut total_score = 0.0_f64;

        for token in &tokens {
            let m = fuzzy_match(token, text);
            if m.matches {
                total_score += m.score;
            } else {
                continue 'items;
            }
        }

        results.push((item.clone(), total_score));
    }

    results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    results.into_iter().map(|r| r.0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let result = fuzzy_match("hello", "hello");
        assert!(result.matches);
        assert!(result.score < 0.0); // exact match gets bonus
    }

    #[test]
    fn test_subsequence_match() {
        let result = fuzzy_match("hlo", "hello");
        assert!(result.matches);
    }

    #[test]
    fn test_no_match() {
        let result = fuzzy_match("xyz", "hello");
        assert!(!result.matches);
    }

    #[test]
    fn test_empty_query() {
        let result = fuzzy_match("", "anything");
        assert!(result.matches);
    }

    #[test]
    fn test_word_boundary_bonus() {
        let boundary = fuzzy_match("fb", "fooBar");
        let no_boundary = fuzzy_match("ob", "fooBar");
        assert!(boundary.matches);
        assert!(no_boundary.matches);
        assert!(boundary.score < no_boundary.score);
    }

    #[test]
    fn test_fuzzy_filter() {
        let items = vec!["hello world", "help", "world", "foo"];
        let results = fuzzy_filter(&items, "he", |s| s);
        assert_eq!(results.len(), 2);
        assert!(results.contains(&"hello world"));
        assert!(results.contains(&"help"));
    }

    #[test]
    fn test_fuzzy_filter_multi_token() {
        let items = vec!["hello world", "hello", "world hello", "foo"];
        let results = fuzzy_filter(&items, "he wo", |s| s);
        assert_eq!(results.len(), 2);
        assert!(results.contains(&"hello world"));
        assert!(results.contains(&"world hello"));
    }
}
