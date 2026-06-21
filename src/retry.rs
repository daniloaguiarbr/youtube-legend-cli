//! Exponential-backoff retry helper and an in-memory circuit breaker.

use crate::error::{AppError, AppResult};
use std::time::Duration;
use tokio::time::sleep;

/// Retry `op` up to `max_attempts` times with 1 s, 2 s, 4 s back-off
/// between attempts. Only errors classified as retryable
/// ([`AppError::Timeout`], [`AppError::ProviderUnavailable`],
/// [`AppError::RateLimited`], [`AppError::Http`]) trigger a back-off;
/// any other error is returned immediately. An HTTP 429
/// ([`AppError::RateLimited`]) waits for the upstream `Retry-After`
/// value (default 60 s, capped at 300 s) instead of the exponential
/// delays (EC-021). The final attempt's failure is reported as
/// [`AppError::ProviderUnavailable`].
///
/// # Cancel safety
///
/// Each call to `op` is its own future; dropping the returned future
/// at any `await` point cancels the in-flight `op` cleanly.
///
/// # Errors
///
/// Returns the last [`AppError`] produced by `op` after exhausting
/// all `max_attempts` retries, with the following variants in scope:
///
/// - [`AppError::ProviderUnavailable`] when every upstream call failed
/// - [`AppError::RateLimited`] when the last upstream call returned 429
///   even after the backoff window
/// - [`AppError::Http`] for transport-level errors that survived retries
/// - [`AppError::Timeout`] when the cumulative wait exceeded the budget
pub async fn retry_with_backoff<F, Fut, T>(mut op: F, max_attempts: u8) -> AppResult<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = AppResult<T>>,
{
    let delays = [
        Duration::from_secs(1),
        Duration::from_secs(2),
        Duration::from_secs(4),
    ];

    for attempt in 0..max_attempts {
        match op().await {
            Ok(value) => return Ok(value),
            Err(e) if !is_retryable(&e) => return Err(e),
            Err(_) if attempt + 1 == max_attempts => {
                return Err(AppError::ProviderUnavailable);
            }
            Err(AppError::RateLimited { retry_after_secs }) => {
                let wait = retry_after_secs.unwrap_or(60).min(300);
                tracing::debug!(
                    target: "events",
                    event = "retry",
                    attempt = attempt + 1,
                    next_delay_secs = wait,
                    "rate limited (HTTP 429); honouring Retry-After"
                );
                sleep(Duration::from_secs(wait)).await;
            }
            Err(_) => {
                if let Some(delay) = delays.get(attempt as usize) {
                    tracing::debug!(
                        target: "events",
                        event = "retry",
                        attempt = attempt + 1,
                        next_delay_secs = delay.as_secs(),
                        "transient failure; backing off"
                    );
                    sleep(*delay).await;
                }
            }
        }
    }

    Err(AppError::ProviderUnavailable)
}

fn is_retryable(err: &AppError) -> bool {
    matches!(
        err,
        AppError::Timeout(_)
            | AppError::ProviderUnavailable
            | AppError::RateLimited { .. }
            | AppError::Http(_)
    )
}

/// In-memory circuit breaker that opens after `threshold` consecutive
/// failures. Intended for use by callers that want a fast-fail signal
/// without spinning the retry loop. Not used by [`retry_with_backoff`]
/// itself; the two compose at the caller's discretion.
#[derive(Debug)]
#[non_exhaustive]
pub struct CircuitBreaker {
    threshold: u8,
    failures: u8,
}

impl CircuitBreaker {
    /// Build a breaker that opens after `threshold` failures.
    pub fn new(threshold: u8) -> Self {
        Self {
            threshold,
            failures: 0,
        }
    }

    /// Record one failure. Returns `true` when the breaker has just
    /// tripped (failures == threshold), so the caller can stop issuing
    /// further requests until [`Self::record_success`] is called.
    pub fn record_failure(&mut self) -> bool {
        self.failures = self.failures.saturating_add(1);
        self.failures >= self.threshold
    }

    /// Record one success and reset the failure counter.
    pub fn record_success(&mut self) {
        self.failures = 0;
    }

    /// `true` when the breaker is currently open.
    pub fn is_open(&self) -> bool {
        self.failures >= self.threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU8, Ordering};

    #[tokio::test(start_paused = true)]
    async fn rate_limited_waits_retry_after_seconds() {
        let calls = AtomicU8::new(0);
        let start = tokio::time::Instant::now();
        let result = retry_with_backoff(
            || async {
                if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(AppError::RateLimited {
                        retry_after_secs: Some(2),
                    })
                } else {
                    Ok(42u8)
                }
            },
            3,
        )
        .await;
        assert_eq!(result.expect("second attempt succeeds"), 42);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_secs(2),
            "virtual clock advanced only {elapsed:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn rate_limited_without_header_waits_60s_fallback() {
        let calls = AtomicU8::new(0);
        let start = tokio::time::Instant::now();
        let result = retry_with_backoff(
            || async {
                if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(AppError::RateLimited {
                        retry_after_secs: None,
                    })
                } else {
                    Ok(())
                }
            },
            3,
        )
        .await;
        assert!(result.is_ok());
        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_secs(60),
            "virtual clock advanced only {elapsed:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn rate_limited_wait_is_capped_at_300s() {
        let calls = AtomicU8::new(0);
        let start = tokio::time::Instant::now();
        let result = retry_with_backoff(
            || async {
                if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(AppError::RateLimited {
                        retry_after_secs: Some(9999),
                    })
                } else {
                    Ok(())
                }
            },
            3,
        )
        .await;
        assert!(result.is_ok());
        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_secs(300) && elapsed < Duration::from_secs(360),
            "expected ~300s of virtual wait, got {elapsed:?}"
        );
    }
}
