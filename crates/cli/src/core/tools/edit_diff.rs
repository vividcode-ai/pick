use serde::Deserialize;

/// Line ending detection
pub fn detect_line_ending(content: &str) -> &str {
    let crlf_idx = content.find("\r\n");
    let lf_idx = content.find('\n');
    match (crlf_idx, lf_idx) {
        (Some(crlf), Some(lf)) => {
            if crlf < lf {
                "\r\n"
            } else {
                "\n"
            }
        }
        (Some(_), None) => "\r\n",
        (None, Some(_)) => "\n",
        (None, None) => "\n",
    }
}

/// Normalize line endings to LF
pub fn normalize_to_lf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Restore original line endings
pub fn restore_line_endings(text: &str, ending: &str) -> String {
    if ending == "\r\n" {
        text.replace('\n', "\r\n")
    } else {
        text.to_string()
    }
}

/// Normalize text for fuzzy matching
pub fn normalize_for_fuzzy_match(text: &str) -> String {
    text
        // Strip trailing whitespace per line
        .lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        // Smart quotes to ASCII
        .replace('\u{2018}', "'")
        .replace('\u{2019}', "'")
        .replace('\u{201A}', "'")
        .replace('\u{201B}', "'")
        .replace('\u{201C}', "\"")
        .replace('\u{201D}', "\"")
        .replace('\u{201E}', "\"")
        .replace('\u{201F}', "\"")
        // Dashes to hyphen
        .replace('\u{2010}', "-")
        .replace('\u{2011}', "-")
        .replace('\u{2012}', "-")
        .replace('\u{2013}', "-")
        .replace('\u{2014}', "-")
        .replace('\u{2015}', "-")
        .replace('\u{2212}', "-")
        // Special spaces to regular space
        .replace('\u{00A0}', " ")
        .replace('\u{2002}', " ")
        .replace('\u{2003}', " ")
        .replace('\u{2004}', " ")
        .replace('\u{2005}', " ")
        .replace('\u{2006}', " ")
        .replace('\u{2007}', " ")
        .replace('\u{2008}', " ")
        .replace('\u{2009}', " ")
        .replace('\u{200A}', " ")
        .replace('\u{202F}', " ")
        .replace('\u{205F}', " ")
        .replace('\u{3000}', " ")
}

/// Result of a fuzzy match
pub struct FuzzyMatchResult {
    pub found: bool,
    pub index: usize,
    pub match_length: usize,
    pub used_fuzzy_match: bool,
    pub content_for_replacement: String,
}

/// A single edit operation
#[derive(Debug, Clone, Deserialize)]
pub struct Edit {
    pub old_text: String,
    pub new_text: String,
}

/// Find oldText in content, trying exact match first, then fuzzy match
pub fn fuzzy_find_text(content: &str, old_text: &str) -> FuzzyMatchResult {
    // Try exact match first
    if let Some(index) = content.find(old_text) {
        return FuzzyMatchResult {
            found: true,
            index,
            match_length: old_text.len(),
            used_fuzzy_match: false,
            content_for_replacement: content.to_string(),
        };
    }

    // Try fuzzy match
    let fuzzy_content = normalize_for_fuzzy_match(content);
    let fuzzy_old_text = normalize_for_fuzzy_match(old_text);
    if let Some(index) = fuzzy_content.find(&fuzzy_old_text) {
        return FuzzyMatchResult {
            found: true,
            index,
            match_length: fuzzy_old_text.len(),
            used_fuzzy_match: true,
            content_for_replacement: fuzzy_content,
        };
    }

    FuzzyMatchResult {
        found: false,
        index: 0,
        match_length: 0,
        used_fuzzy_match: false,
        content_for_replacement: content.to_string(),
    }
}

/// Strip UTF-8 BOM if present
pub fn strip_bom(content: &str) -> (String, String) {
    if content.starts_with('\u{FEFF}') {
        ("\u{FEFF}".to_string(), content[1..].to_string())
    } else {
        (String::new(), content.to_string())
    }
}

/// Result of applying edits
pub struct AppliedEditsResult {
    pub base_content: String,
    pub new_content: String,
}

fn count_occurrences(content: &str, old_text: &str) -> usize {
    let fuzzy_content = normalize_for_fuzzy_match(content);
    let fuzzy_old_text = normalize_for_fuzzy_match(old_text);
    fuzzy_content.matches(&fuzzy_old_text).count()
}

struct MatchedEdit {
    edit_index: usize,
    match_index: usize,
    match_length: usize,
    new_text: String,
}

/// Apply one or more exact-text replacements to LF-normalized content
pub fn apply_edits_to_normalized_content(
    normalized_content: &str,
    edits: &[Edit],
    path: &str,
) -> Result<AppliedEditsResult, String> {
    let normalized_edits: Vec<Edit> = edits
        .iter()
        .map(|e| Edit {
            old_text: normalize_to_lf(&e.old_text),
            new_text: normalize_to_lf(&e.new_text),
        })
        .collect();

    for (i, edit) in normalized_edits.iter().enumerate() {
        if edit.old_text.is_empty() {
            return if normalized_edits.len() == 1 {
                Err(format!("oldText must not be empty in {}.", path))
            } else {
                Err(format!(
                    "edits[{}].oldText must not be empty in {}.",
                    i, path
                ))
            };
        }
    }

    let initial_matches: Vec<FuzzyMatchResult> = normalized_edits
        .iter()
        .map(|edit| fuzzy_find_text(normalized_content, &edit.old_text))
        .collect();

    let use_fuzzy = initial_matches.iter().any(|m| m.used_fuzzy_match);
    let base_content = if use_fuzzy {
        normalize_for_fuzzy_match(normalized_content)
    } else {
        normalized_content.to_string()
    };

    let mut matched_edits: Vec<MatchedEdit> = Vec::new();
    for (i, edit) in normalized_edits.iter().enumerate() {
        let match_result = fuzzy_find_text(&base_content, &edit.old_text);
        if !match_result.found {
            return if normalized_edits.len() == 1 {
                Err(format!(
                    "Could not find the exact text in {}. The old text must match exactly including all whitespace and newlines.",
                    path
                ))
            } else {
                Err(format!(
                    "Could not find edits[{}] in {}. The oldText must match exactly including all whitespace and newlines.",
                    i, path
                ))
            };
        }

        let occurrences = count_occurrences(&base_content, &edit.old_text);
        if occurrences > 1 {
            return if normalized_edits.len() == 1 {
                Err(format!(
                    "Found {} occurrences of the text in {}. The text must be unique. Please provide more context to make it unique.",
                    occurrences, path
                ))
            } else {
                Err(format!(
                    "Found {} occurrences of edits[{}] in {}. Each oldText must be unique. Please provide more context to make it unique.",
                    occurrences, i, path
                ))
            };
        }

        matched_edits.push(MatchedEdit {
            edit_index: i,
            match_index: match_result.index,
            match_length: match_result.match_length,
            new_text: edit.new_text.clone(),
        });
    }

    matched_edits.sort_by_key(|m| m.match_index);
    for i in 1..matched_edits.len() {
        let prev = &matched_edits[i - 1];
        let curr = &matched_edits[i];
        if prev.match_index + prev.match_length > curr.match_index {
            return Err(format!(
                "edits[{}] and edits[{}] overlap in {}. Merge them into one edit or target disjoint regions.",
                prev.edit_index, curr.edit_index, path
            ));
        }
    }

    let mut new_content = base_content.clone();
    for edit in matched_edits.iter().rev() {
        new_content = format!(
            "{}{}{}",
            &new_content[..edit.match_index],
            edit.new_text,
            &new_content[edit.match_index + edit.match_length..]
        );
    }

    if base_content == new_content {
        return if normalized_edits.len() == 1 {
            Err(format!(
                "No changes made to {}. The replacement produced identical content. This might indicate an issue with special characters or the text not existing as expected.",
                path
            ))
        } else {
            Err(format!(
                "No changes made to {}. The replacements produced identical content.",
                path
            ))
        };
    }

    Ok(AppliedEditsResult {
        base_content,
        new_content,
    })
}

/// Generate a diff string with line numbers
pub fn generate_diff_string(
    old_content: &str,
    new_content: &str,
    context_lines: usize,
) -> DiffResult {
    let old_lines: Vec<&str> = if old_content.is_empty() {
        Vec::new()
    } else {
        old_content.split('\n').collect()
    };
    let new_lines: Vec<&str> = if new_content.is_empty() {
        Vec::new()
    } else {
        new_content.split('\n').collect()
    };
    let max_line_num = old_lines.len().max(new_lines.len());
    let line_num_width = max_line_num.to_string().len();

    let mut output: Vec<String> = Vec::new();
    let mut old_line_num = 1usize;
    let mut new_line_num = 1usize;
    let mut first_changed_line: Option<usize> = None;
    let mut last_was_change = false;

    // Use a simple LCS-based diff on lines
    let diff_ops = similar::TextDiff::from_lines(old_content, new_content);
    let mut ops: Vec<(similar::ChangeTag, usize, usize, &str)> = Vec::new();
    for change in diff_ops.iter_all_changes() {
        let tag = change.tag();
        let value = change.value();
        let old_idx = change.old_index().unwrap_or(0);
        let new_idx = change.new_index().unwrap_or(0);
        ops.push((tag, old_idx, new_idx, value));
    }

    let mut i = 0;
    while i < ops.len() {
        let (tag, _old_idx, _new_idx, value) = &ops[i];

        match tag {
            similar::ChangeTag::Equal => {
                let raw = value.trim_end_matches('\n');
                if raw.is_empty()
                    && (i == ops.len() - 1 || ops[i + 1].0 == similar::ChangeTag::Equal)
                {
                    // Skip trailing empty context
                    break;
                }

                let mut eq_lines: Vec<&str> = if raw.is_empty() {
                    Vec::new()
                } else {
                    raw.split('\n').collect()
                };
                if value.ends_with('\n')
                    && !eq_lines.is_empty()
                    && eq_lines.last().copied() == Some("")
                {
                    eq_lines.pop();
                }

                // Check if next ops are changes
                let next_is_change = i + 1 < ops.len()
                    && (ops[i + 1].0 == similar::ChangeTag::Insert
                        || ops[i + 1].0 == similar::ChangeTag::Delete);

                let prev_is_change = last_was_change;

                if prev_is_change && next_is_change {
                    if eq_lines.len() <= context_lines * 2 {
                        for line in &eq_lines {
                            let ln = format!("{:>width$}", old_line_num, width = line_num_width);
                            output.push(format!(" {} {}", ln, line));
                            old_line_num += 1;
                            new_line_num += 1;
                        }
                    } else {
                        let leading = &eq_lines[..eq_lines.len().min(context_lines)];
                        let trailing_start = eq_lines.len().saturating_sub(context_lines);
                        let trailing = &eq_lines[trailing_start..];

                        for line in leading {
                            let ln = format!("{:>width$}", old_line_num, width = line_num_width);
                            output.push(format!(" {} {}", ln, line));
                            old_line_num += 1;
                            new_line_num += 1;
                        }
                        let pad = " ".repeat(line_num_width);
                        output.push(format!(" {} ...", pad));
                        old_line_num += eq_lines.len() - leading.len() - trailing.len();
                        new_line_num += eq_lines.len() - leading.len() - trailing.len();
                        for line in trailing {
                            let ln = format!("{:>width$}", old_line_num, width = line_num_width);
                            output.push(format!(" {} {}", ln, line));
                            old_line_num += 1;
                            new_line_num += 1;
                        }
                    }
                } else if prev_is_change {
                    let shown = &eq_lines[..eq_lines.len().min(context_lines)];
                    let skipped = eq_lines.len() - shown.len();
                    for line in shown {
                        let ln = format!("{:>width$}", old_line_num, width = line_num_width);
                        output.push(format!(" {} {}", ln, line));
                        old_line_num += 1;
                        new_line_num += 1;
                    }
                    if skipped > 0 {
                        let pad = " ".repeat(line_num_width);
                        output.push(format!(" {} ...", pad));
                        old_line_num += skipped;
                        new_line_num += skipped;
                    }
                } else if next_is_change {
                    let skipped = eq_lines.len().saturating_sub(context_lines);
                    if skipped > 0 {
                        let pad = " ".repeat(line_num_width);
                        output.push(format!(" {} ...", pad));
                        old_line_num += skipped;
                        new_line_num += skipped;
                    }
                    for line in eq_lines.iter().skip(skipped) {
                        let ln = format!("{:>width$}", old_line_num, width = line_num_width);
                        output.push(format!(" {} {}", ln, line));
                        old_line_num += 1;
                        new_line_num += 1;
                    }
                } else {
                    old_line_num += eq_lines.len();
                    new_line_num += eq_lines.len();
                }
                last_was_change = false;
            }
            similar::ChangeTag::Delete => {
                if first_changed_line.is_none() {
                    first_changed_line = Some(new_line_num);
                }
                let raw = value.trim_end_matches('\n');
                if !raw.is_empty() {
                    for line in raw.split('\n') {
                        let ln = format!("{:>width$}", old_line_num, width = line_num_width);
                        output.push(format!("-{} {}", ln, line));
                        old_line_num += 1;
                    }
                }
                last_was_change = true;
            }
            similar::ChangeTag::Insert => {
                if first_changed_line.is_none() {
                    first_changed_line = Some(new_line_num);
                }
                let raw = value.trim_end_matches('\n');
                if !raw.is_empty() {
                    for line in raw.split('\n') {
                        let ln = format!("{:>width$}", new_line_num, width = line_num_width);
                        output.push(format!("+{} {}", ln, line));
                        new_line_num += 1;
                    }
                }
                last_was_change = true;
            }
        }
        i += 1;
    }

    DiffResult {
        diff: output.join("\n"),
        first_changed_line,
    }
}

pub struct DiffResult {
    pub diff: String,
    pub first_changed_line: Option<usize>,
}

/// Compute the diff for one or more edit operations without applying them
pub async fn compute_edits_diff(
    path: &str,
    edits: &[Edit],
    cwd: &str,
) -> Result<DiffResult, String> {
    let absolute_path = super::path_utils::resolve_to_cwd(path, cwd);

    // Check if file exists and is readable
    if !super::path_utils::path_exists(&absolute_path).await {
        return Err(format!("Could not edit file: {}.", path));
    }

    // Read the file
    let content = tokio::fs::read_to_string(&absolute_path)
        .await
        .map_err(|e| format!("Could not edit file: {}. {}", path, e))?;

    let (_bom, text) = strip_bom(&content);
    let normalized_content = normalize_to_lf(&text);
    let result =
        apply_edits_to_normalized_content(&normalized_content, edits, path).map_err(|e| e)?;

    Ok(generate_diff_string(
        &result.base_content,
        &result.new_content,
        4,
    ))
}
