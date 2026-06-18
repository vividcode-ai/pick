use std::time::Duration;

/// Check if a reqwest transport error is retryable (network-level failures)
pub fn is_retryable_request_error(e: &reqwest::Error) -> bool {
    if e.is_timeout() || e.is_connect() || e.is_request() {
        return true;
    }
    let err_str = e.to_string().to_lowercase();
    err_str.contains("connection refused")
        || err_str.contains("connection reset")
        || err_str.contains("connection closed")
        || err_str.contains("dns")
        || err_str.contains("tls")
        || err_str.contains("timeout")
        || err_str.contains("eof")
        || err_str.contains("broken pipe")
        || err_str.contains("reset before headers")
        || err_str.contains("socket hang up")
        || err_str.contains("http2")
        || err_str.contains("stream ended")
}

/// Check if an HTTP status code is retryable
pub fn is_retryable_http_status(status: u16) -> bool {
    matches!(status, 429 | 502 | 503 | 504)
}

/// Calculate delay with exponential backoff (+25% jitter)
///
/// - attempt=1: base_delay_ms * 1 + jitter
/// - attempt=2: base_delay_ms * 2 + jitter
/// - attempt=3: base_delay_ms * 4 + jitter
/// - attempt=N: base_delay_ms * 2^(N-1) + jitter
/// Capped at max_delay_ms.
pub fn retry_delay(attempt: u32, base_delay_ms: u64, max_delay_ms: u64) -> Duration {
    if attempt == 0 {
        return Duration::from_millis(0);
    }
    let exp = base_delay_ms * 2u64.pow(attempt.saturating_sub(1));
    let delay = exp.min(max_delay_ms);
    let jitter = delay / 4;
    let jitter_offset = (attempt as u64 * 137) % (jitter + 1);
    Duration::from_millis(delay + jitter_offset)
}

/// Whether we should retry given the current attempt and max_retries from options
pub fn should_retry(attempt: u32, max_retries: u32) -> bool {
    attempt < max_retries
}
