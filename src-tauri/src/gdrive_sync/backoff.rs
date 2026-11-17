use std::time::Duration;

use reqwest::header::HeaderMap;

pub fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    let val = headers.get("retry-after")?.to_str().ok()?.trim();
    val.parse::<u64>().ok().map(Duration::from_secs)
}

/// Computes exponential backoff with full jitter in [0, min(cap, base * 2^attempt)].
pub fn jittered_backoff(attempt: u32, base_ms: u64, cap_ms: u64) -> Duration {
    let shift = attempt.min(10);
    let max_ms = base_ms.saturating_mul(1u64 << shift).min(cap_ms).max(1);
    let mut rnd = [0u8; 8];
    let _ = getrandom::fill(&mut rnd);
    let val = u64::from_le_bytes(rnd) % max_ms;
    Duration::from_millis(val.max(50))
}

pub fn is_retryable_status(status: reqwest::StatusCode, body: &str) -> bool {
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        return true;
    }
    if status == reqwest::StatusCode::FORBIDDEN
        && (body.contains("rateLimitExceeded") || body.contains("userRateLimitExceeded"))
    {
        return true;
    }
    false
}
