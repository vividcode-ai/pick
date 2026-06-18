//! Edit tool - applies string replacements to files with fuzzy matching

use pick_ai::types::ContentBlock;

use crate::core::state::{AgentTool, AgentToolResult, ToolExecutionMode};

// ============================================================================
// Unicode normalisation for fuzzy matching
// ============================================================================

/// Normalise text for lenient matching: NFKC + quote/dash/space normalisation
fn normalise_for_fuzzy_match(text: &str) -> String {
    text.lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        // Smart quotes → ASCII
        .replace('\u{2018}', "'")
        .replace('\u{2019}', "'")
        .replace('\u{201A}', "'")
        .replace('\u{201B}', "'")
        .replace('\u{201C}', "\"")
        .replace('\u{201D}', "\"")
        .replace('\u{201E}', "\"")
        .replace('\u{201F}', "\"")
        // Dashes → hyphen
        .replace('\u{2010}', "-")
        .replace('\u{2011}', "-")
        .replace('\u{2012}', "-")
        .replace('\u{2013}', "-")
        .replace('\u{2014}', "-")
        .replace('\u{2015}', "-")
        .replace('\u{2212}', "-")
        // Special spaces → regular space
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

/// Try to find old_text in content — exact match first, then fuzzy
fn fuzzy_find_text(content: &str, old_text: &str) -> FuzzyMatch {
    if let Some(index) = content.find(old_text) {
        return FuzzyMatch {
            found: true,
            index,
            match_length: old_text.len(),
            used_fuzzy: false,
            content_for_replacement: content.to_string(),
        };
    }
    let fuzzy_c = normalise_for_fuzzy_match(content);
    let fuzzy_o = normalise_for_fuzzy_match(old_text);
    if let Some(index) = fuzzy_c.find(&fuzzy_o) {
        return FuzzyMatch {
            found: true,
            index,
            match_length: fuzzy_o.len(),
            used_fuzzy: true,
            content_for_replacement: fuzzy_c,
        };
    }
    FuzzyMatch {
        found: false,
        index: 0,
        match_length: 0,
        used_fuzzy: false,
        content_for_replacement: content.to_string(),
    }
}

struct FuzzyMatch {
    found: bool,
    index: usize,
    match_length: usize,
    used_fuzzy: bool,
    content_for_replacement: String,
}

fn count_occurrences(content: &str, old_text: &str) -> usize {
    let fc = normalise_for_fuzzy_match(content);
    let fo = normalise_for_fuzzy_match(old_text);
    fc.matches(&fo).count()
}

// ============================================================================
// Line-ending helpers
// ============================================================================

fn normalise_to_lf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn strip_bom(content: &str) -> (String, String) {
    if content.starts_with('\u{FEFF}') {
        ("\u{FEFF}".to_string(), content[1..].to_string())
    } else {
        (String::new(), content.to_string())
    }
}

// ============================================================================
// Apply multiple edits
// ============================================================================

struct MatchedEdit {
    match_index: usize,
    match_length: usize,
    new_text: String,
}

fn apply_edits(
    content: &str,
    edits: &[(String, String)],
    file_path: &str,
) -> Result<(String, String), String> {
    let norm_edits: Vec<(String, String)> = edits
        .iter()
        .map(|(o, n)| (normalise_to_lf(o), normalise_to_lf(n)))
        .collect();

    for (i, (old, _)) in norm_edits.iter().enumerate() {
        if old.is_empty() {
            let label = if edits.len() == 1 {
                "oldText".to_string()
            } else {
                format!("edits[{}].oldText", i)
            };
            return Err(format!("{} must not be empty in {}.", label, file_path));
        }
    }

    let initial: Vec<FuzzyMatch> = norm_edits
        .iter()
        .map(|(o, _)| fuzzy_find_text(content, o))
        .collect();

    let use_fuzzy = initial.iter().any(|m| m.used_fuzzy);
    let base = if use_fuzzy {
        normalise_for_fuzzy_match(content)
    } else {
        content.to_string()
    };

    let mut matched: Vec<MatchedEdit> = Vec::new();
    for (i, (old, new)) in norm_edits.iter().enumerate() {
        let m = fuzzy_find_text(&base, old);
        if !m.found {
            let label = if edits.len() == 1 {
                "oldText".to_string()
            } else {
                format!("edits[{}]", i)
            };
            return Err(format!(
                "Could not find the exact text for {} in {}. \
                 The text must match exactly including all whitespace and newlines. \
                 Try providing more context or use a fuzzy match.",
                label, file_path
            ));
        }
        let occ = count_occurrences(&base, old);
        if occ > 1 {
            let label = if edits.len() == 1 {
                "oldText".to_string()
            } else {
                format!("edits[{}]", i)
            };
            return Err(format!(
                "Found {} occurrences of {} in {}. \
                 The text must be unique. Please provide more context to make it unique.",
                occ, label, file_path
            ));
        }
        matched.push(MatchedEdit {
            match_index: m.index,
            match_length: m.match_length,
            new_text: new.clone(),
        });
    }

    matched.sort_by_key(|m| m.match_index);
    for i in 1..matched.len() {
        let prev = &matched[i - 1];
        let cur = &matched[i];
        if prev.match_index + prev.match_length > cur.match_index {
            let label = if edits.len() == 1 {
                "edits[0] and edits[0]".to_string()
            } else {
                format!("edits[{}] and edits[{}]", i - 1, i)
            };
            return Err(format!(
                "{} overlap in {}. Merge them into one edit.",
                label, file_path
            ));
        }
    }

    let mut result = base.clone();
    for m in matched.iter().rev() {
        result = format!(
            "{}{}{}",
            &result[..m.match_index],
            m.new_text,
            &result[m.match_index + m.match_length..]
        );
    }

    if base == result {
        return Err(format!(
            "No changes made to {}. The replacement produced identical content.",
            file_path
        ));
    }

    Ok((base, result))
}

// ============================================================================
// Diff formatting
// ============================================================================

fn format_diff(file_path: &str, base: &str, new: &str) -> String {
    let mut out = String::from("Successfully applied edit\n");
    out.push_str(&format!("--- {}", file_path));
    for line in base.split('\n') {
        out.push_str(&format!("\n-{}", line));
    }
    out.push_str(&format!("\n+++ {}", file_path));
    for line in new.split('\n') {
        out.push_str(&format!("\n+{}", line));
    }
    out
}

// ============================================================================
// Tool definition
// ============================================================================

/// Create the edit tool definition with multi-edit and fuzzy matching support
pub fn create_edit_tool() -> AgentTool {
    let params = pick_ai::types::JsonSchema {
        schema_type: "object".to_string(),
        properties: Some(vec![
            (
                    "path".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Path to the file to edit (relative or absolute)"
                    }),
                ),
                (
                    "file_path".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Alternative name for 'path'"
                    }),
                ),
                (
                    "oldText".to_string(),
                serde_json::json!({
                    "type": "string",
                    "description": "[DEPRECATED] Exact text for a targeted replacement. Prefer using edits[] instead."
                }),
            ),
            (
                "newText".to_string(),
                serde_json::json!({
                    "type": "string",
                    "description": "[DEPRECATED] Replacement text. Prefer using edits[] instead."
                }),
            ),
            (
                "edits".to_string(),
                serde_json::json!({
                    "type": "array",
                    "description": "One or more targeted replacements. Each edit is matched against the original file, not incrementally.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "oldText": { "type": "string", "description": "Exact text for one targeted replacement. Must be unique in the file." },
                            "newText": { "type": "string", "description": "Replacement text for this targeted edit." }
                        },
                        "required": ["oldText", "newText"],
                        "additionalProperties": false
                    }
                }),
            ),
        ].into_iter().collect()),
        required: Some(vec!["path".to_string()]),
        description: Some(
            "Edit a file using exact text replacement. ".to_string()
        ),
        items: None,
        additional_properties: Some(false),
    };

    AgentTool {
        name: "edit".to_string(),
        description: "Edit a file using exact text replacement. Each edit is applied against the original file content, not incrementally. Supports Unicode-aware fuzzy matching as fallback.".to_string(),
        prompt_snippet: Some("Make precise file edits with exact text replacement, including multiple disjoint edits in one call".to_string()),
        prompt_guidelines: vec![
            "Use edit for precise changes (edits[].oldText must match exactly)".to_string(),
            "When changing multiple separate locations in one file, use one edit call with multiple entries in edits[] instead of multiple edit calls".to_string(),
            "Each edits[].oldText is matched against the original file, not after earlier edits are applied. Do not emit overlapping or nested edits. Merge nearby changes into one edit.".to_string(),
            "Keep edits[].oldText as small as possible while still being unique in the file. Do not pad with large unchanged regions.".to_string(),
        ],
        label: "edit".to_string(),
        parameters: params,
        execute: std::sync::Arc::new(|_tool_call_id, args, ctx| {
            Box::pin(async move {
                let file_path = args.get("file_path").or_else(|| args.get("path")).and_then(|v| v.as_str()).ok_or_else(|| "Missing path".to_string())?;

                if let (Some(ref policy), Some(ref cwd)) = (ctx.fs_policy, ctx.cwd) {
                    if let Err(_e) = policy.can_write(std::path::Path::new(file_path), cwd) {
                        // Protected paths (e.g. .git/**) are hard denied, not authorizable
                        if policy.is_path_protected(std::path::Path::new(file_path), cwd).unwrap_or(false) {
                            return Ok(AgentToolResult {
                                content: vec![ContentBlock::text(format!("FsPolicy: {}", _e))],
                                is_error: true, terminate: false,
                            });
                        }
                        // External paths: check authorization
                        if let Some(ref pm) = ctx.permission_manager {
                            let authorized = crate::permission::external_dir::check_authorization(
                                "Edit", file_path, pm, ctx.question.as_ref(),
                            ).await?;
                            if !authorized {
                                return Ok(AgentToolResult {
                                    content: vec![ContentBlock::text(format!(
                                        "Error: Write access denied: '{}' is outside the allowed workspace", file_path
                                    ))],
                                    is_error: true, terminate: false,
                                });
                            }
                        } else {
                            return Ok(AgentToolResult {
                                content: vec![ContentBlock::text(format!("FsPolicy: {}", _e))],
                                is_error: true, terminate: false,
                            });
                        }
                    }
                }

                // Collect edits from either the single oldText/newText or the edits[] array
                let mut edits: Vec<(String, String)> = Vec::new();

                if let Some(edits_val) = args.get("edits") {
                    // Try to parse as array
                    if let Some(arr) = edits_val.as_array() {
                        for (i, edit) in arr.iter().enumerate() {
                            let old = edit.get("oldText").and_then(|v| v.as_str()).ok_or_else(|| {
                                format!("Missing oldText in edits[{}]", i)
                            })?;
                            let new = edit.get("newText").and_then(|v| v.as_str()).ok_or_else(|| {
                                format!("Missing newText in edits[{}]", i)
                            })?;
                            edits.push((old.to_string(), new.to_string()));
                        }
                    } else if let Some(s) = edits_val.as_str() {
                        // Some models send edits as a JSON string — parse it
                        let parsed: Vec<serde_json::Value> = serde_json::from_str(s)
                            .map_err(|e| format!("Failed to parse edits JSON string: {}", e))?;
                        for (i, edit) in parsed.iter().enumerate() {
                            let old = edit.get("oldText").and_then(|v| v.as_str()).ok_or_else(|| {
                                format!("Missing oldText in parsed edits[{}]", i)
                            })?;
                            let new = edit.get("newText").and_then(|v| v.as_str()).ok_or_else(|| {
                                format!("Missing newText in parsed edits[{}]", i)
                            })?;
                            edits.push((old.to_string(), new.to_string()));
                        }
                    }
                }

                // Fall back to single oldText/newText for backward compat
                if edits.is_empty() {
                    let old_text = args.get("oldText").and_then(|v| v.as_str()).ok_or_else(|| {
                        "Missing oldText. Provide either 'oldText' or 'edits'.".to_string()
                    })?;
                    let new_text = args.get("newText").and_then(|v| v.as_str()).ok_or_else(|| {
                        "Missing newText. Provide either 'newText' or 'edits'.".to_string()
                    })?;
                    edits.push((old_text.to_string(), new_text.to_string()));
                }

                // Read and normalise the file
                let raw_content = tokio::fs::read_to_string(file_path).await
                    .map_err(|e| format!("Error reading file: {}", e))?;

                let (_bom, body) = strip_bom(&raw_content);
                let normalised = normalise_to_lf(&body);

                // Apply edits
                let (base_content, new_content) = apply_edits(&normalised, &edits, file_path)?;

                // Build the final content with BOM and original line endings restored
                let original_ending = detect_line_ending(&raw_content);
                let final_content = if original_ending == "\r\n" {
                    _bom + &new_content.replace('\n', "\r\n")
                } else {
                    _bom + &new_content
                };

                tokio::fs::write(file_path, &final_content).await
                    .map_err(|e| format!("Error writing file: {}", e))?;

                let diff_output = format_diff(file_path, &base_content, &new_content);

                Ok(AgentToolResult {
                    content: vec![ContentBlock::text(diff_output)],
                    is_error: false,
                    terminate: false,
                })
            })
        }),
        execution_mode: ToolExecutionMode::Sequential,
    }
}

fn detect_line_ending(content: &str) -> &str {
    let crlf = content.find("\r\n");
    let lf = content.find('\n');
    match (crlf, lf) {
        (Some(cr), Some(lf)) if cr < lf => "\r\n",
        (Some(_), None) => "\r\n",
        _ => "\n",
    }
}
