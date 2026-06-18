//! HTTP dispatcher configuration for API requests


pub const DEFAULT_HTTP_IDLE_TIMEOUT_MS: u64 = 300_000;

pub const HTTP_IDLE_TIMEOUT_CHOICES: &[(u64, &str)] = &[
    (30_000, "30 sec"),
    (60_000, "1 min"),
    (120_000, "2 min"),
    (300_000, "5 min"),
    (0, "disabled"),
];

/// Parse an HTTP idle timeout value from string or number
pub fn parse_http_idle_timeout_ms(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("disabled") {
        return Some(0);
    }
    if trimmed.is_empty() {
        return None;
    }
    let num: u64 = trimmed.parse().ok()?;
    Some(num)
}

/// Format HTTP idle timeout milliseconds to a display label
pub fn format_http_idle_timeout_ms(timeout_ms: u64) -> String {
    for &(ms, label) in HTTP_IDLE_TIMEOUT_CHOICES {
        if ms == timeout_ms {
            return label.to_string();
        }
    }
    format!("{} sec", timeout_ms / 1000)
}
