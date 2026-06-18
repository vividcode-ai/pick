//! Session analysis utilities
//! Provides cost analysis, token statistics, and tool usage analysis
//! for session data.

use std::collections::HashMap;
use std::path::Path;

/// Per-day/provider cost breakdown
#[derive(Debug, Clone, Default)]
pub struct DayCost {
    pub total: f64,
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    pub requests: usize,
}

/// Session usage statistics
#[derive(Debug, Clone, Default)]
pub struct UsageStats {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub total_tokens: u64,
    pub cost_input: f64,
    pub cost_output: f64,
    pub cost_cache_read: f64,
    pub cost_cache_write: f64,
    pub cost_total: f64,
    pub assistant_messages: usize,
}

/// Token bucket boundaries for histogram display
pub const TOKEN_BUCKETS: &[u64] = &[
    0,
    50,
    100,
    250,
    500,
    1000,
    2000,
    4000,
    8000,
    16000,
    32000,
    u64::MAX,
];

/// Format a number with locale-style separators
pub fn format_int(value: u64) -> String {
    let s = value.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Format a cost value as a dollar string
pub fn format_cost(value: f64) -> String {
    format!("${:.4}", value)
}

/// Aggregate day cost data into a formatted summary string
pub fn format_cost_summary(
    day: &str,
    providers: &HashMap<String, DayCost>,
    day_total: f64,
) -> String {
    let mut output = format!("\n{}\n", day);
    output.push_str(&"-".repeat(40));
    output.push('\n');

    let mut providers_sorted: Vec<(&String, &DayCost)> = providers.iter().collect();
    providers_sorted.sort_by(|a, b| a.0.cmp(b.0));

    for (provider, stats) in &providers_sorted {
        output.push_str(&format!(
            "  {:15} ${:>8.4}  ({} reqs, in: ${:.4}, out: ${:.4}, cache: ${:.4})\n",
            provider,
            stats.total,
            stats.requests,
            stats.input,
            stats.output,
            stats.cache_read + stats.cache_write,
        ));
    }

    output.push_str(&format!("  {:15} ${:>8.4}\n", "Day total:", day_total));
    output
}

/// Aggregate usage stats into a formatted line
pub fn format_usage_line(label: &str, stats: &UsageStats) -> String {
    format!(
        "{:16} messages: {:5}  input: {:>12}  output: {:>10}  \
         cache read: {:>13}  cache write: {:>10}  \
         total: {:>13}  cost: {:>10}",
        label,
        stats.assistant_messages,
        format_int(stats.input_tokens),
        format_int(stats.output_tokens),
        format_int(stats.cache_read_tokens),
        format_int(stats.cache_write_tokens),
        format_int(stats.total_tokens),
        format_cost(stats.cost_total),
    )
}

/// Estimate token count from text length (1 token ≈ 4 characters)
pub fn estimate_token_count(text: &str) -> u64 {
    let len = text.len();
    (len as f64 / 4.0).ceil() as u64
}

/// Extract the root command key from a bash command string.
pub fn command_key(command: &str) -> String {
    // Split by operators and take first command segment
    let first = command
        .split(|c| c == '\n' || c == '&' || c == '|' || c == ';')
        .next()
        .map(|s| s.trim())
        .unwrap_or(command.trim());

    // Regex: skip env vars (KEY=VALUE), skip sudo, capture binary and optional subcommand
    let re = regex::Regex::new(r"^(?:\w+=\S+\s+)*(?:sudo\s+)?([^\s]+)(?:\s+([^\s]+))?").unwrap();
    if let Some(caps) = re.captures(first) {
        let bin = caps.get(1).map(|m| m.as_str()).unwrap_or("unknown");
        let sub = caps.get(2).map(|m| m.as_str());
        match sub {
            Some(s) if !s.starts_with('-') => format!("{} {}", bin, s),
            _ => bin.to_string(),
        }
    } else {
        "unknown".to_string()
    }
}

/// Compute histogram bucket counts from a list of samples
pub fn bucket_counts(samples: &[u64]) -> Vec<usize> {
    let mut counts = vec![0usize; TOKEN_BUCKETS.len() - 1];
    for &sample in samples {
        for (i, window) in TOKEN_BUCKETS.windows(2).enumerate() {
            if sample >= window[0] && sample < window[1] {
                counts[i] += 1;
                break;
            }
        }
    }
    counts
}

/// Generate bucket labels for histogram display
pub fn bucket_labels() -> Vec<String> {
    TOKEN_BUCKETS[..TOKEN_BUCKETS.len() - 1]
        .iter()
        .enumerate()
        .map(|(i, &min)| {
            let max = TOKEN_BUCKETS[i + 1];
            if max == u64::MAX {
                format!("{}+", min)
            } else {
                format!("{}-{}", min, max)
            }
        })
        .collect()
}

/// Encode a directory path to a session directory name
pub fn encode_session_dir(cwd: &str) -> String {
    let normalized = if let Some(stripped) = cwd.strip_prefix('/') {
        stripped
    } else if let Some(stripped) = cwd.strip_prefix("\\\\?\\") {
        stripped
    } else if let Some(stripped) = cwd.strip_prefix("C:") {
        stripped
    } else if let Some(stripped) = cwd.strip_prefix("D:") {
        stripped
    } else {
        cwd
    };
    format!("--{}--", normalized.replace('/', "-").replace('\\', "-"))
}

/// Get a local date key string (YYYY-MM-DD) from a timestamp
pub fn local_day_key(timestamp_millis: i64) -> String {
    let secs = timestamp_millis / 1000;
    let datetime = chrono::DateTime::from_timestamp(secs, 0).unwrap_or_default();
    datetime.format("%Y-%m-%d").to_string()
}

/// Parse a session file path and extract session entries.
/// Session files use JSONL format with one JSON object per line.
pub fn parse_session_file(path: &Path) -> Result<Vec<serde_json::Value>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    let mut entries = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(line) {
            Ok(entry) => entries.push(entry),
            Err(_) => continue, // Skip malformed lines
        }
    }
    Ok(entries)
}

/// Extract assistant message usage info from a session entry.
/// Returns (provider, usage_cost) if the entry is an assistant message with usage data.
pub fn extract_message_cost(entry: &serde_json::Value) -> Option<(String, DayCost)> {
    let entry_type = entry.get("type")?.as_str()?;
    if entry_type != "message" {
        return None;
    }

    let message = entry.get("message")?;
    let role = message.get("role")?.as_str()?;
    if role != "assistant" {
        return None;
    }

    let usage = message.get("usage")?;
    let cost = usage.get("cost")?;

    let provider = message
        .get("provider")
        .and_then(|p| p.as_str())
        .unwrap_or("unknown")
        .to_string();

    Some((
        provider,
        DayCost {
            total: cost.get("total").and_then(|v| v.as_f64()).unwrap_or(0.0),
            input: cost.get("input").and_then(|v| v.as_f64()).unwrap_or(0.0),
            output: cost.get("output").and_then(|v| v.as_f64()).unwrap_or(0.0),
            cache_read: cost
                .get("cacheRead")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            cache_write: cost
                .get("cacheWrite")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            requests: 1,
        },
    ))
}

/// Extract assistant message usage statistics from a session entry.
/// Returns usage stats if the entry is an assistant message with usage data.
pub fn extract_message_usage(entry: &serde_json::Value) -> Option<(String, UsageStats)> {
    let entry_type = entry.get("type")?.as_str()?;
    if entry_type != "message" {
        return None;
    }

    let message = entry.get("message")?;
    let role = message.get("role")?.as_str()?;
    if role != "assistant" {
        return None;
    }

    let usage = message.get("usage")?;
    let cost = usage.get("cost");

    let provider = message
        .get("provider")
        .and_then(|p| p.as_str())
        .unwrap_or("unknown")
        .to_string();

    let input_tokens = usage.get("input").and_then(|v| v.as_u64()).unwrap_or(0);
    let output_tokens = usage.get("output").and_then(|v| v.as_u64()).unwrap_or(0);
    let cache_read = usage.get("cacheRead").and_then(|v| v.as_u64()).unwrap_or(0);
    let cache_write = usage
        .get("cacheWrite")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    Some((
        provider,
        UsageStats {
            input_tokens,
            output_tokens,
            cache_read_tokens: cache_read,
            cache_write_tokens: cache_write,
            total_tokens: input_tokens + output_tokens + cache_read + cache_write,
            cost_input: cost
                .and_then(|c| c.get("input"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            cost_output: cost
                .and_then(|c| c.get("output"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            cost_cache_read: cost
                .and_then(|c| c.get("cacheRead"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            cost_cache_write: cost
                .and_then(|c| c.get("cacheWrite"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            cost_total: cost
                .and_then(|c| c.get("total"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            assistant_messages: 1,
        },
    ))
}

/// Analyze session files in a directory and aggregate costs by day and provider.
pub fn analyze_session_costs(
    sessions_dir: &Path,
    cutoff_days: usize,
) -> Result<HashMap<String, HashMap<String, DayCost>>, String> {
    if !sessions_dir.exists() {
        return Err(format!(
            "Sessions directory not found: {}",
            sessions_dir.display()
        ));
    }

    let cutoff =
        std::time::SystemTime::now() - std::time::Duration::from_secs(cutoff_days as u64 * 86400);

    let mut stats: HashMap<String, HashMap<String, DayCost>> = HashMap::new();

    let dir_iter = std::fs::read_dir(sessions_dir)
        .map_err(|e| format!("Failed to read sessions directory: {}", e))?;

    for entry in dir_iter {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }

        // Extract timestamp from filename: <timestamp>_<uuid>.jsonl
        let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let ts_part = filename.split('_').next().unwrap_or("");

        // Parse ISO timestamp format: 2025-12-17T08-25-07-381Z
        let is_ts = ts_part
            .replace("T08-25-07-381Z", "T08:25:07.381Z")
            .replace('Z', "+00:00");
        let file_time = match is_ts.parse::<chrono::DateTime<chrono::Utc>>() {
            Ok(dt) => dt,
            Err(_) => continue,
        };

        if file_time < chrono::DateTime::<chrono::Utc>::from(cutoff) {
            continue;
        }

        let entries = parse_session_file(&path)?;
        for entry_val in entries {
            if let Some((provider, cost)) = extract_message_cost(&entry_val) {
                let day = file_time.format("%Y-%m-%d").to_string();
                let day_entry = stats
                    .entry(day.clone())
                    .or_default()
                    .entry(provider)
                    .or_insert_with(DayCost::default);
                day_entry.total += cost.total;
                day_entry.input += cost.input;
                day_entry.output += cost.output;
                day_entry.cache_read += cost.cache_read;
                day_entry.cache_write += cost.cache_write;
                day_entry.requests += cost.requests;
            }
        }
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_int() {
        assert_eq!(format_int(0), "0");
        assert_eq!(format_int(100), "100");
        assert_eq!(format_int(1000), "1,000");
        assert_eq!(format_int(1000000), "1,000,000");
    }

    #[test]
    fn test_format_cost() {
        assert_eq!(format_cost(0.0), "$0.0000");
        assert_eq!(format_cost(1.5), "$1.5000");
        assert_eq!(format_cost(0.1234), "$0.1234");
    }

    #[test]
    fn test_estimate_token_count() {
        assert_eq!(estimate_token_count("hello"), 2); // 5/4 = 1.25 -> 2
        assert_eq!(estimate_token_count("a"), 1); // 1/4 = 0.25 -> 1
        assert_eq!(estimate_token_count(""), 0); // 0/4 = 0 -> 0
        assert_eq!(estimate_token_count("abcdefgh"), 2); // 8/4 = 2 -> 2
    }

    #[test]
    fn test_command_key_simple() {
        assert_eq!(command_key("ls -la"), "ls");
        assert_eq!(command_key("git status"), "git status");
        assert_eq!(command_key("sudo apt install"), "apt install");
    }

    #[test]
    fn test_command_key_with_env() {
        assert_eq!(command_key("FOO=bar BAR=baz git status"), "git status");
        assert_eq!(command_key("NODE_ENV=production npm run build"), "npm run");
    }

    #[test]
    fn test_command_key_pipeline() {
        // Takes first segment before pipe operator, including its arguments
        let cmd = "cat file.txt | grep pattern | wc -l";
        assert_eq!(command_key(cmd), "cat file.txt");
    }

    #[test]
    fn test_command_key_multiline() {
        // Takes first segment before newline/operator
        let cmd = "echo hello\n&& echo world\n|| true";
        assert_eq!(command_key(cmd), "echo hello");
    }

    #[test]
    fn test_command_key_unknown() {
        assert_eq!(command_key(""), "unknown");
        assert_eq!(command_key("   "), "unknown");
    }

    #[test]
    fn test_bucket_counts() {
        let samples = vec![10, 75, 200, 600, 5000];
        let counts = bucket_counts(&samples);
        assert_eq!(counts.len(), TOKEN_BUCKETS.len() - 1);
        // 10 falls in bucket 0 (0-50)
        assert_eq!(counts[0], 1);
        // 75 falls in bucket 1 (50-100)
        assert_eq!(counts[1], 1);
        // 200 falls in bucket 2 (100-250)
        assert_eq!(counts[2], 1);
        // 600 falls in bucket 4 (500-1000)
        assert_eq!(counts[4], 1);
        // 5000 falls in bucket 6 (2000-4000)? No, 5000 > 4000, so bucket 7 (4000-8000)
        assert_eq!(counts[7], 1);
    }

    #[test]
    fn test_bucket_labels() {
        let labels = bucket_labels();
        assert_eq!(labels.len(), TOKEN_BUCKETS.len() - 1);
        assert_eq!(labels[0], "0-50");
        assert!(labels.last().unwrap().ends_with('+'));
    }

    #[test]
    fn test_encode_session_dir() {
        let encoded = encode_session_dir("/home/user/project");
        assert!(encoded.contains("home-user-project"));
        assert!(encoded.starts_with("--"));
        assert!(encoded.ends_with("--"));
    }

    #[test]
    fn test_encode_session_dir_windows() {
        let encoded = encode_session_dir("D:\\project\\my-app");
        assert!(encoded.contains("project-my-app"));
    }

    #[test]
    fn test_local_day_key() {
        let ts = 1700000000000; // 2023-11-14T22:13:20Z
        let key = local_day_key(ts);
        assert_eq!(key.len(), 10); // YYYY-MM-DD
    }

    #[test]
    fn test_extract_message_cost_no_entry() {
        let val = serde_json::json!({"type": "other"});
        assert!(extract_message_cost(&val).is_none());
    }

    #[test]
    fn test_extract_message_cost_no_usage() {
        let val = serde_json::json!({
            "type": "message",
            "message": {
                "role": "user",
                "content": "hello"
            }
        });
        assert!(extract_message_cost(&val).is_none());
    }

    #[test]
    fn test_extract_message_cost_with_cost() {
        let val = serde_json::json!({
            "type": "message",
            "message": {
                "role": "assistant",
                "provider": "anthropic",
                "usage": {
                    "cost": {
                        "total": 0.015,
                        "input": 0.01,
                        "output": 0.005,
                        "cacheRead": 0.0,
                        "cacheWrite": 0.0
                    }
                }
            }
        });
        let (provider, cost) = extract_message_cost(&val).unwrap();
        assert_eq!(provider, "anthropic");
        assert!((cost.total - 0.015).abs() < 0.001);
        assert!((cost.input - 0.01).abs() < 0.001);
    }

    #[test]
    fn test_extract_message_usage() {
        let val = serde_json::json!({
            "type": "message",
            "message": {
                "role": "assistant",
                "provider": "openai",
                "usage": {
                    "input": 500,
                    "output": 200,
                    "cacheRead": 100,
                    "cacheWrite": 50,
                    "cost": {
                        "total": 0.01,
                        "input": 0.005,
                        "output": 0.003,
                        "cacheRead": 0.001,
                        "cacheWrite": 0.001
                    }
                }
            }
        });
        let (provider, stats) = extract_message_usage(&val).unwrap();
        assert_eq!(provider, "openai");
        assert_eq!(stats.input_tokens, 500);
        assert_eq!(stats.output_tokens, 200);
        assert_eq!(stats.total_tokens, 850);
        assert!((stats.cost_total - 0.01).abs() < 0.001);
    }
}
