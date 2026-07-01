use std::path::Path;

use axum::Json;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use tracing::warn;
use utoipa::ToSchema;

/// Query params for reading a file
#[derive(Deserialize, ToSchema)]
pub struct FileReadParams {
    pub path: String,
    pub offset: Option<u64>,
    pub limit: Option<u64>,
}

/// Query params for listing a directory
#[derive(Deserialize, ToSchema)]
pub struct ListDirParams {
    pub path: String,
    pub limit: Option<u64>,
}

/// Query params for text search
#[derive(Deserialize, ToSchema)]
pub struct FindTextParams {
    pub pattern: String,
    pub path: Option<String>,
    #[serde(rename = "glob")]
    pub glob_filter: Option<String>,
    #[serde(rename = "ignoreCase")]
    pub ignore_case: Option<bool>,
    pub literal: Option<bool>,
    pub context: Option<u64>,
    pub limit: Option<u64>,
}

/// Query params for file search
#[derive(Deserialize, ToSchema)]
pub struct FindFilesParams {
    pub pattern: String,
    pub path: Option<String>,
    pub limit: Option<u64>,
}

/// File entry for directory listing
#[derive(Serialize, ToSchema)]
pub struct FileEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub size: Option<u64>,
    pub modified: Option<i64>,
}

/// Directory listing response
#[derive(Serialize, ToSchema)]
pub struct ListDirResponse {
    pub path: String,
    pub entries: Vec<FileEntry>,
    pub truncated: bool,
}

/// File read response
#[derive(Serialize)]
pub struct FileReadResponse {
    pub path: String,
    pub content: String,
    pub total_lines: Option<usize>,
    pub binary: bool,
}

/// Search match
#[derive(Clone, Serialize, ToSchema)]
pub struct SearchMatch {
    pub path: String,
    pub line: u64,
    pub content: String,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

/// Text search response
#[derive(Serialize, ToSchema)]
pub struct FindTextResponse {
    pub matches: Vec<SearchMatch>,
    pub total: usize,
    pub truncated: bool,
}

/// File search result
#[derive(Clone, Serialize, ToSchema)]
pub struct FileSearchResult {
    pub path: String,
}

/// File search response
#[derive(Serialize, ToSchema)]
pub struct FindFilesResponse {
    pub files: Vec<FileSearchResult>,
    pub total: usize,
    pub truncated: bool,
}

/// Read a file's contents
#[utoipa::path(
    get,
    path = "/files/content",
    tag = "files",
    params(
        ("path" = String, Query, description = "Path to the file"),
        ("offset" = Option<u64>, Query, description = "Line number to start from (1-indexed)"),
        ("limit" = Option<u64>, Query, description = "Maximum number of lines to read"),
    ),
    responses(
        (status = 200, description = "File content"),
        (status = 404, description = "File not found"),
    )
)]
pub async fn read_file_handler(Query(params): Query<FileReadParams>) -> impl IntoResponse {
    let path = Path::new(&params.path);

    // Security: prevent path traversal
    if params.path.contains("..") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Path traversal detected"})),
        )
            .into_response();
    }

    if !path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "File not found"})),
        )
            .into_response();
    }

    if !path.is_file() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Not a file"})),
        )
            .into_response();
    }

    // Try to detect binary by reading first few bytes
    let _is_binary = {
        let data = match tokio::fs::read(&params.path).await {
            Ok(d) => d,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": format!("Failed to read file: {}", e)})),
                )
                    .into_response();
            }
        };
        let is_bin = data[..data.len().min(1024)].iter().any(|&b| b == 0x00);
        if is_bin {
            return (
                StatusCode::OK,
                Json(serde_json::json!({
                    "path": params.path,
                    "content": format!("(binary file, {} bytes)", data.len()),
                    "total_lines": null,
                    "binary": true,
                    "size": data.len(),
                })),
            )
                .into_response();
        }
        false
    };

    match tokio::fs::read_to_string(&params.path).await {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let total_lines = lines.len();

            let offset = params.offset.unwrap_or(1).saturating_sub(1) as usize;
            let limit = params.limit.unwrap_or(u64::MAX) as usize;
            let selected: Vec<&str> = lines.iter().copied().skip(offset).take(limit).collect();
            let result = selected.join("\n");

            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "path": params.path,
                    "content": result,
                    "total_lines": total_lines,
                    "binary": false,
                    "size": content.len(),
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to read file: {}", e)})),
        )
            .into_response(),
    }
}

/// List directory contents
#[utoipa::path(
    get,
    path = "/files/list",
    tag = "files",
    params(
        ("path" = String, Query, description = "Directory path"),
        ("limit" = Option<u64>, Query, description = "Maximum entries to return"),
    ),
    responses(
        (status = 200, description = "Directory listing", body = ListDirResponse),
        (status = 404, description = "Path not found"),
    )
)]
pub async fn list_dir_handler(Query(params): Query<ListDirParams>) -> impl IntoResponse {
    let path = Path::new(&params.path);

    if params.path.contains("..") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Path traversal detected"})),
        )
            .into_response();
    }

    if !path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Path not found"})),
        )
            .into_response();
    }

    if !path.is_dir() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Not a directory"})),
        )
            .into_response();
    }

    let mut entries = match tokio::fs::read_dir(path).await {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to read directory: {}", e)})),
            )
                .into_response();
        }
    };

    let limit = params.limit.unwrap_or(500) as usize;
    let mut file_entries = Vec::new();
    let mut truncated = false;

    while let Some(entry) = entries.next_entry().await.transpose() {
        match entry {
            Ok(entry) => {
                if file_entries.len() >= limit {
                    truncated = true;
                    break;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                let file_type = match entry.file_type().await {
                    Ok(ft) => {
                        if ft.is_dir() {
                            "directory"
                        } else if ft.is_symlink() {
                            "symlink"
                        } else {
                            "file"
                        }
                    }
                    Err(_) => "unknown",
                };
                let size = match entry.metadata().await {
                    Ok(m) => Some(m.len()),
                    Err(_) => None,
                };
                let modified = match entry.metadata().await {
                    Ok(m) => m.modified().ok().map(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .ok()
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0)
                    }),
                    Err(_) => None,
                };
                file_entries.push(FileEntry {
                    name,
                    entry_type: file_type.to_string(),
                    size,
                    modified,
                });
            }
            Err(e) => {
                warn!("Error reading directory entry: {}", e);
            }
        }
    }

    file_entries.sort_by(|a, b| {
        let a_type = if a.entry_type == "directory" { 0 } else { 1 };
        let b_type = if b.entry_type == "directory" { 0 } else { 1 };
        a_type.cmp(&b_type).then(a.name.cmp(&b.name))
    });

    (
        StatusCode::OK,
        Json(ListDirResponse {
            path: params.path,
            entries: file_entries,
            truncated,
        }),
    )
        .into_response()
}

/// Search file contents for a pattern
#[utoipa::path(
    get,
    path = "/find/text",
    tag = "files",
    params(
        ("pattern" = String, Query, description = "Search pattern (regex or literal)"),
        ("path" = Option<String>, Query, description = "Directory to search"),
        ("glob" = Option<String>, Query, description = "Glob filter for files"),
        ("ignoreCase" = Option<bool>, Query, description = "Case-insensitive search"),
        ("literal" = Option<bool>, Query, description = "Treat pattern as literal string"),
        ("context" = Option<u64>, Query, description = "Context lines before/after match"),
        ("limit" = Option<u64>, Query, description = "Max matches"),
    ),
    responses(
        (status = 200, description = "Search results", body = FindTextResponse),
        (status = 404, description = "Search path not found"),
    )
)]
pub async fn find_text_handler(Query(params): Query<FindTextParams>) -> impl IntoResponse {
    let search_path = params.path.as_deref().unwrap_or(".");
    let path = Path::new(search_path);

    if !path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Search path not found"})),
        )
            .into_response();
    }

    let ignore_case = params.ignore_case.unwrap_or(false);
    let literal = params.literal.unwrap_or(false);
    let context_lines = params.context.unwrap_or(0) as usize;
    let limit = params.limit.unwrap_or(100) as usize;

    // Build regex pattern
    let raw = if literal {
        regex::escape(&params.pattern)
    } else {
        params.pattern.clone()
    };

    let re = match if ignore_case {
        regex::RegexBuilder::new(&raw)
            .case_insensitive(true)
            .build()
    } else {
        regex::Regex::new(&raw)
    } {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Invalid pattern: {}", e)})),
            )
                .into_response();
        }
    };

    // Build glob filter if specified
    let glob_re = params
        .glob_filter
        .as_ref()
        .and_then(|g| regex::Regex::new(&glob_to_regex(g)).ok());

    let mut matches = Vec::new();
    let mut count = 0;

    let walker = walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'));

    for entry in walker.filter_map(|e| e.ok()) {
        if count >= limit {
            break;
        }
        if !entry.file_type().is_file() {
            continue;
        }

        // Apply glob filter
        if let Some(ref gre) = glob_re {
            let rel = entry.path().strip_prefix(path).unwrap_or(entry.path());
            if !gre.is_match(&rel.to_string_lossy()) {
                continue;
            }
        }

        let content = match std::fs::read_to_string(entry.path()) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let file_path = entry.path().display().to_string();
        let lines: Vec<&str> = content.lines().collect();

        for (linenum, line) in lines.iter().enumerate() {
            if count >= limit {
                break;
            }
            if re.is_match(line) {
                let ctx_before: Vec<String> = if context_lines > 0 {
                    let start = linenum.saturating_sub(context_lines);
                    lines[start..linenum]
                        .iter()
                        .enumerate()
                        .map(|(i, l)| format!("{}:{}", start + i + 1, l))
                        .collect()
                } else {
                    Vec::new()
                };

                let ctx_after: Vec<String> = if context_lines > 0 {
                    let end = std::cmp::min(linenum + context_lines + 1, lines.len());
                    lines[linenum + 1..end]
                        .iter()
                        .enumerate()
                        .map(|(i, l)| format!("{}:{}", linenum + i + 2, l))
                        .collect()
                } else {
                    Vec::new()
                };

                matches.push(SearchMatch {
                    path: file_path.clone(),
                    line: linenum as u64 + 1,
                    content: (*line).to_string(),
                    context_before: ctx_before,
                    context_after: ctx_after,
                });
                count += 1;
            }
        }
    }

    (
        StatusCode::OK,
        Json(FindTextResponse {
            matches,
            total: count,
            truncated: count >= limit,
        }),
    )
        .into_response()
}

/// Search files by glob pattern
#[utoipa::path(
    get,
    path = "/find/files",
    tag = "files",
    params(
        ("pattern" = String, Query, description = "Glob pattern (e.g. '*.rs')"),
        ("path" = Option<String>, Query, description = "Directory to search"),
        ("limit" = Option<u64>, Query, description = "Max results"),
    ),
    responses(
        (status = 200, description = "Matching files", body = FindFilesResponse),
        (status = 404, description = "Search path not found"),
    )
)]
pub async fn find_files_handler(Query(params): Query<FindFilesParams>) -> impl IntoResponse {
    let search_path = params.path.as_deref().unwrap_or(".");
    let path = Path::new(search_path);

    if !path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Search path not found"})),
        )
            .into_response();
    }

    let limit = params.limit.unwrap_or(1000) as usize;
    let regex_str = glob_to_regex(&params.pattern);

    let re = match regex::Regex::new(&regex_str) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Invalid glob pattern: {}", e)})),
            )
                .into_response();
        }
    };

    let mut files = Vec::new();

    let walker = walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'));

    for entry in walker.filter_map(|e| e.ok()) {
        if files.len() >= limit {
            break;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let full_path = entry.path();
        let relative = full_path.strip_prefix(path).unwrap_or(full_path);
        let name = relative.to_string_lossy();
        if re.is_match(&name) {
            files.push(FileSearchResult {
                path: format!("./{}", name),
            });
        }
    }

    let total = files.len();
    (
        StatusCode::OK,
        Json(FindFilesResponse {
            files,
            total,
            truncated: total >= limit,
        }),
    )
        .into_response()
}

/// Simple glob-to-regex conversion (reused from pick_agent::tools::find)
fn glob_to_regex(glob: &str) -> String {
    let mut re = String::with_capacity(glob.len() + 4);
    re.push('^');
    let mut chars = glob.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next();
                    re.push_str(".*");
                    if chars.peek() == Some(&'/') {
                        chars.next();
                    }
                } else {
                    re.push_str("[^/]*");
                }
            }
            '?' => re.push_str("[^/]"),
            '.' => re.push_str("\\."),
            '/' => re.push_str("[/\\\\]"),
            '\\' => re.push_str("\\\\"),
            '+' => re.push_str("\\+"),
            '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '!' => {
                re.push('\\');
                re.push(ch);
            }
            c => re.push(c),
        }
    }
    re.push('$');
    re
}
