//! Subtitle provider trait, two concrete implementations, and a
//! throttled provider chain.
//!
//! A `Provider` is the unit of pluggable I/O against one upstream
//! subtitle source. A `ProviderChain` walks its providers in order,
//! honours a one-request-per-second throttle, and aggregates the
//! results into a single `SubtitleInfo` + body pair.

pub mod provider_a;
pub mod provider_b;
#[cfg(feature = "headless")]
pub mod provider_headless;
pub mod robots;

pub use provider_a::ProviderA;
pub use provider_b::ProviderB;
#[cfg(feature = "headless")]
pub use provider_headless::ProviderHeadless;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::error::{AppError, AppResult};

/// Classify a non-success upstream HTTP status (EC-021): HTTP 429 maps
/// to [`AppError::RateLimited`] carrying the parsed `Retry-After`
/// value; every other failure maps to
/// [`AppError::ProviderUnavailable`]. `Retry-After` is accepted both
/// as delta-seconds and as an RFC 2822 HTTP-date; a date in the past
/// yields zero (no wait) so clock skew never produces a bogus delay.
/// Unparseable values are treated as absent, so the retry layer falls
/// back to 60 s.
pub(crate) fn http_failure(
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
) -> AppError {
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let retry_after_secs = headers
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_retry_after);
        return AppError::RateLimited { retry_after_secs };
    }
    AppError::ProviderUnavailable
}

/// Parse a `Retry-After` header value: delta-seconds first, then an
/// RFC 2822 HTTP-date converted to seconds from now, clamped to zero
/// when the date is already in the past.
fn parse_retry_after(raw: &str) -> Option<u64> {
    let s = raw.trim();
    if let Ok(secs) = s.parse::<u64>() {
        return Some(secs);
    }
    let dt = chrono::DateTime::parse_from_rfc2822(s).ok()?;
    let delta = (dt.with_timezone(&chrono::Utc) - chrono::Utc::now()).num_seconds();
    Some(delta.max(0) as u64)
}

/// Record a provider failure in `last_err` without letting a later
/// generic failure overwrite an earlier [`AppError::RateLimited`]
/// (EC-021): the `Retry-After` information must survive the chain so
/// the retry layer can honour it.
fn remember_failure(last_err: &mut Option<AppError>, e: AppError) {
    let downgrade = matches!(last_err, Some(AppError::RateLimited { .. }))
        && !matches!(e, AppError::RateLimited { .. });
    if !downgrade {
        *last_err = Some(e);
    }
}

/// Subtitle delivery format.
///
/// `Srt` preserves the raw `SubRip` text; `Txt` strips timestamps and
/// joins cues with blank lines. The variant is serialised in the
/// `--json` envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Format {
    /// `SubRip` text with timestamps preserved.
    Srt,
    /// Plain text with timestamps removed.
    Txt,
}

impl Format {
    /// Lowercase string identifier (`"srt"` or `"txt"`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Format::Srt => "srt",
            Format::Txt => "txt",
        }
    }

    /// File extension associated with the format. Identical to
    /// [`Format::as_str`] for the current variants, but kept as a
    /// separate method so future variants can diverge.
    pub fn extension(&self) -> &'static str {
        match self {
            Format::Srt => "srt",
            Format::Txt => "txt",
        }
    }
}

/// Metadata for a single subtitle retrieval, returned by
/// [`Provider::fetch_subtitle`] before the body is fetched by
/// [`Provider::fetch_content`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SubtitleInfo {
    /// 11-character `YouTube` video id.
    pub video_id: String,
    /// ISO 639-1 language code that matched.
    pub language: String,
    /// Delivery format the body will be in.
    pub format: Format,
    /// Provider-supplied URL to the raw subtitle body.
    pub source_url: String,
    /// Body size in bytes, populated by `fetch_content` (zero before).
    pub byte_size: usize,
}

/// Pluggable subtitle source. Implementations must be `Send + Sync` so
/// they can be stored in a `Box<dyn Provider>` inside a [`ProviderChain`].
#[doc(alias = "Source")]
#[doc(alias = "Upstream")]
#[doc(alias = "Backend")]
#[doc(alias = "pluggable")]
#[doc(alias = "trait")]
#[doc(alias = "async trait")]
#[doc(alias = "subtitle source")]
#[doc(alias = "upstream")]
#[async_trait]
pub trait Provider: Send + Sync {
    /// Short human-readable identifier, used in tracing events.
    fn name(&self) -> &'static str;

    /// Resolve the subtitle URL and language match for a given video.
    ///
    /// # Errors
    ///
    /// - [`AppError::NoSubtitle`] when the provider has nothing for the
    ///   request (a structured [`crate::error::NoSubtitleReason`] is
    ///   attached).
    /// - [`AppError::ProviderUnavailable`] on transient upstream
    ///   failure.
    /// - [`AppError::Http`] on transport errors.
    async fn fetch_subtitle(
        &self,
        video_id: &str,
        language: &str,
        format: Format,
    ) -> AppResult<SubtitleInfo>;

    /// Download the body bytes for the given [`SubtitleInfo`].
    ///
    /// # Errors
    ///
    /// - [`AppError::NoSubtitle`] when the body is empty or the URL has
    ///   gone stale.
    /// - [`AppError::ProviderUnavailable`] on transient upstream
    ///   failure.
    async fn fetch_content(&self, info: &SubtitleInfo) -> AppResult<Vec<u8>>;
}

/// Walks a list of providers in order, honouring a per-call minimum
/// interval, until one returns a non-empty body. All providers in the
/// chain must outlive the chain.
pub struct ProviderChain {
    providers: Vec<Box<dyn Provider>>,
    min_interval: Duration,
    last_call: Mutex<Option<Instant>>,
}

impl ProviderChain {
    /// Build a chain with the default one-request-per-second throttle.
    #[tracing::instrument(level = "debug", skip_all, fields(providers = providers.len()))]
    pub fn new(providers: Vec<Box<dyn Provider>>) -> Self {
        Self::with_min_interval(providers, Duration::from_secs(1))
    }

    /// Build a chain with a custom minimum interval between calls.
    #[tracing::instrument(level = "debug", skip_all, fields(min_interval_ms = %min_interval.as_millis()))]
    pub fn with_min_interval(providers: Vec<Box<dyn Provider>>, min_interval: Duration) -> Self {
        Self {
            providers,
            min_interval,
            last_call: Mutex::new(None),
        }
    }

    /// Sleep just long enough to honour the configured `min_interval`,
    /// then record the current instant. Call this before every
    /// `fetch_subtitle` or `fetch_content` invocation.
    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn throttle(&self) {
        let now = Instant::now();
        let wait = {
            let guard = self
                .last_call
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard
                .map(|t| {
                    let elapsed = now.duration_since(t);
                    if elapsed < self.min_interval {
                        Some(self.min_interval - elapsed)
                    } else {
                        None
                    }
                })
                .unwrap_or(None)
        };
        if let Some(d) = wait {
            tokio::time::sleep(d).await;
        }
        *self
            .last_call
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Instant::now());
    }

    /// Try every provider in order, returning the first non-empty body.
    /// If at least one provider answered with a structured
    /// [`AppError::NoSubtitle`], that result wins over a generic
    /// [`AppError::ProviderUnavailable`].
    ///
    /// # Errors
    ///
    /// - [`AppError::NoSubtitle`] if every provider reported the absence
    ///   of a subtitle.
    /// - [`AppError::RateLimited`] if any provider answered HTTP 429
    ///   and no later provider succeeded; the `Retry-After` value is
    ///   preserved across the chain (EC-021).
    /// - [`AppError::ProviderUnavailable`] if every provider failed
    ///   transiently and none reported a structured reason.
    #[tracing::instrument(level = "debug", err, skip(self), fields(video_id, language, format = ?format))]
    pub async fn fetch_subtitle(
        &self,
        video_id: &str,
        language: &str,
        format: Format,
    ) -> AppResult<(SubtitleInfo, Vec<u8>)> {
        let mut last_err: Option<AppError> = None;
        let mut saw_no_subtitle = false;
        for provider in &self.providers {
            self.throttle().await;
            match provider.fetch_subtitle(video_id, language, format).await {
                Ok(info) => match provider.fetch_content(&info).await {
                    Ok(content) if !content.is_empty() => return Ok((info, content)),
                    Ok(_) => {
                        saw_no_subtitle = true;
                    }
                    Err(e) => remember_failure(&mut last_err, e),
                },
                Err(AppError::NoSubtitle(reason)) => {
                    tracing::warn!(target: "events", reason = %reason, "provider returned no subtitle");
                    saw_no_subtitle = true;
                }
                Err(e) => remember_failure(&mut last_err, e),
            }
        }

        if saw_no_subtitle {
            return Err(AppError::NoSubtitle(
                crate::error::NoSubtitleReason::NotPublished,
            ));
        }

        Err(last_err.unwrap_or(AppError::ProviderUnavailable))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_failure_maps_429_with_retry_after() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, "7".parse().expect("ascii"));
        let err = http_failure(reqwest::StatusCode::TOO_MANY_REQUESTS, &headers);
        assert!(matches!(
            err,
            AppError::RateLimited {
                retry_after_secs: Some(7)
            }
        ));
    }

    #[test]
    fn http_failure_maps_429_without_header() {
        let headers = reqwest::header::HeaderMap::new();
        let err = http_failure(reqwest::StatusCode::TOO_MANY_REQUESTS, &headers);
        assert!(matches!(
            err,
            AppError::RateLimited {
                retry_after_secs: None
            }
        ));
    }

    #[test]
    fn http_failure_parses_http_date_retry_after() {
        let future = (chrono::Utc::now() + chrono::Duration::seconds(120)).to_rfc2822();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, future.parse().expect("ascii"));
        let err = http_failure(reqwest::StatusCode::TOO_MANY_REQUESTS, &headers);
        match err {
            AppError::RateLimited {
                retry_after_secs: Some(n),
            } => assert!((115..=120).contains(&n), "delta out of range: {n}"),
            other => panic!("expected RateLimited with seconds, got {other:?}"),
        }
    }

    #[test]
    fn http_failure_http_date_in_past_clamps_to_zero() {
        let past = (chrono::Utc::now() - chrono::Duration::seconds(3600)).to_rfc2822();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::RETRY_AFTER, past.parse().expect("ascii"));
        let err = http_failure(reqwest::StatusCode::TOO_MANY_REQUESTS, &headers);
        assert!(matches!(
            err,
            AppError::RateLimited {
                retry_after_secs: Some(0)
            }
        ));
    }

    #[test]
    fn http_failure_garbage_retry_after_is_none() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::RETRY_AFTER,
            "not-a-date-or-number".parse().expect("ascii"),
        );
        let err = http_failure(reqwest::StatusCode::TOO_MANY_REQUESTS, &headers);
        assert!(matches!(
            err,
            AppError::RateLimited {
                retry_after_secs: None
            }
        ));
    }

    #[test]
    fn http_failure_maps_other_status_to_unavailable() {
        let headers = reqwest::header::HeaderMap::new();
        let err = http_failure(reqwest::StatusCode::SERVICE_UNAVAILABLE, &headers);
        assert!(matches!(err, AppError::ProviderUnavailable));
    }

    #[test]
    fn rate_limited_survives_later_transient_failure() {
        let mut last = Some(AppError::RateLimited {
            retry_after_secs: Some(5),
        });
        remember_failure(&mut last, AppError::ProviderUnavailable);
        assert!(matches!(
            last,
            Some(AppError::RateLimited {
                retry_after_secs: Some(5)
            })
        ));
        remember_failure(
            &mut last,
            AppError::RateLimited {
                retry_after_secs: None,
            },
        );
        assert!(matches!(
            last,
            Some(AppError::RateLimited {
                retry_after_secs: None
            })
        ));
    }

    #[test]
    fn remember_failure_records_first_error() {
        let mut last = None;
        remember_failure(&mut last, AppError::ProviderUnavailable);
        assert!(matches!(last, Some(AppError::ProviderUnavailable)));
    }

    #[tokio::test]
    async fn wiremock_429_delta_seconds_reaches_http_failure() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(429).insert_header("Retry-After", "2"))
            .mount(&server)
            .await;
        let resp = reqwest::Client::new()
            .get(server.uri())
            .send()
            .await
            .expect("mock request");
        let err = http_failure(resp.status(), resp.headers());
        assert!(matches!(
            err,
            AppError::RateLimited {
                retry_after_secs: Some(2)
            }
        ));
    }

    #[tokio::test]
    async fn wiremock_429_http_date_reaches_http_failure() {
        let future = (chrono::Utc::now() + chrono::Duration::seconds(90)).to_rfc2822();
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(
                wiremock::ResponseTemplate::new(429).insert_header("Retry-After", future.as_str()),
            )
            .mount(&server)
            .await;
        let resp = reqwest::Client::new()
            .get(server.uri())
            .send()
            .await
            .expect("mock request");
        let err = http_failure(resp.status(), resp.headers());
        match err {
            AppError::RateLimited {
                retry_after_secs: Some(n),
            } => assert!((85..=90).contains(&n), "delta out of range: {n}"),
            other => panic!("expected RateLimited with seconds, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn chain_propagates_rate_limited_over_later_503() {
        struct MockStatusProvider {
            url: String,
        }
        #[async_trait]
        impl Provider for MockStatusProvider {
            fn name(&self) -> &'static str {
                "mock-status"
            }
            async fn fetch_subtitle(
                &self,
                _video_id: &str,
                _language: &str,
                _format: Format,
            ) -> AppResult<SubtitleInfo> {
                let resp = reqwest::Client::new()
                    .get(&self.url)
                    .send()
                    .await
                    .map_err(AppError::Http)?;
                Err(http_failure(resp.status(), resp.headers()))
            }
            async fn fetch_content(&self, _info: &SubtitleInfo) -> AppResult<Vec<u8>> {
                Err(AppError::ProviderUnavailable)
            }
        }

        let rate_limited = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(429).insert_header("Retry-After", "3"))
            .mount(&rate_limited)
            .await;
        let unavailable = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(503))
            .mount(&unavailable)
            .await;

        let chain = ProviderChain::with_min_interval(
            vec![
                Box::new(MockStatusProvider {
                    url: rate_limited.uri(),
                }),
                Box::new(MockStatusProvider {
                    url: unavailable.uri(),
                }),
            ],
            Duration::from_millis(1),
        );
        let err = chain
            .fetch_subtitle("dQw4w9WgXcQ", "en", Format::Srt)
            .await
            .expect_err("both providers fail");
        assert!(matches!(
            err,
            AppError::RateLimited {
                retry_after_secs: Some(3)
            }
        ));
    }

    #[tokio::test]
    async fn chain_throttles_to_one_per_second() {
        let chain = ProviderChain::new(vec![]);
        let start = std::time::Instant::now();
        for _ in 0..3 {
            chain.throttle().await;
        }
        let elapsed = start.elapsed();
        assert!(elapsed >= std::time::Duration::from_millis(1900));
    }
}
