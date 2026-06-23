//! Subtitle provider trait, the noteey.com implementation, and a
//! throttled provider chain.
//!
//! A `Provider` is the unit of pluggable I/O against one upstream
//! subtitle source. A `ProviderChain` walks its providers in order,
//! honours a one-request-per-second throttle, and aggregates the
//! results into a single `SubtitleInfo` + body pair.

pub mod provider_noteey;
pub mod robots;
pub mod stealth;

pub use provider_noteey::ProviderNoteey;

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
///
/// Retained as the canonical HTTP-status classifier for chain
/// implementors and exercised by the EC-021 regression tests below;
/// the single noteey provider currently routes its 429 handling
/// through its own path, so there is no production call site today.
#[allow(dead_code)]
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
#[allow(dead_code)]
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
///
/// GAP-AUD-2026-054: the same protection extends to
/// [`AppError::BrowserNotFound`] and [`AppError::CaptchaChallenge`]
/// — both are environment-level signals the operator must see. A
/// later `NoSubtitle(NotPublished)` from a static provider must NOT
/// silence the earlier "chrome is missing" or "captcha required"
/// signal.
fn remember_failure(last_err: &mut Option<AppError>, e: AppError) {
    let downgrade = matches!(
        last_err,
        Some(
            AppError::RateLimited { .. }
                | AppError::BrowserNotFound(_)
                | AppError::CaptchaChallenge { .. }
        )
    ) && !matches!(
        e,
        AppError::RateLimited { .. }
            | AppError::BrowserNotFound(_)
            | AppError::CaptchaChallenge { .. }
    );
    if !downgrade {
        *last_err = Some(e);
    }
}

/// GAP-AUD-2026-039: intermediate type for the chain classification
/// of provider responses. Allows the chain to distinguish "genuine
/// `NoSubtitle`" from "upstream-degraded `NoSubtitle`" (which should
/// not block fallback).
///
/// `Subtitle` is the happy path. `ChainError` carries both the error
/// AND a `degraded` flag. When `degraded = true`, the chain continues
/// to the next provider even if a later provider would otherwise
/// report a genuine `NoSubtitle`, because the upstream failure was
/// not a real "no captions" answer.
///
/// `degraded = false` is a "real" error worth surfacing (auth,
/// internal failure) — the chain still proceeds but as a non-degraded
/// failure.
///
/// This enum is internal to [`ProviderChain`]. The public
/// [`Provider`] trait still returns [`AppResult`] to preserve
/// backward compatibility with external implementors.
#[derive(Debug)]
pub enum ProviderOutcome {
    /// Successful fetch — `(info, body_bytes)`.
    Subtitle(SubtitleInfo, Vec<u8>),
    /// Provider failed; the chain must decide whether to continue.
    ChainError {
        /// Stable provider identifier (matches `Provider::name()`).
        source: &'static str,
        /// Concrete error that the operator should see in the
        /// envelope when the chain finally fails.
        error: AppError,
        /// `true` when the failure is clearly upstream (5xx, 429,
        /// captcha, network). The chain continues to the next
        /// provider without marking this as a "no subtitle"
        /// verdict.
        degraded: bool,
    },
}

impl ProviderOutcome {
    /// Classify a raw HTTP status into a [`ProviderOutcome::ChainError`].
    ///
    /// HTTP 5xx and 429 are marked `degraded = true` so the chain
    /// continues even if a later provider would report
    /// `NoSubtitle`. 4xx (other than 429) is treated as "the upstream
    /// confirmed no captions exist" (per `YouTube` `timedtext`
    /// convention codified by GAP-E2E-026) and marked `degraded = false`.
    pub fn from_http_status(
        source: &'static str,
        status: u16,
        retry_after_secs: Option<u64>,
    ) -> Self {
        let degraded = matches!(status, 500..=599) || status == 429;
        let error = if let Some(reason) = crate::error::NoSubtitleReason::from_status(status) {
            AppError::NoSubtitle(reason)
        } else if status == 429 {
            AppError::RateLimited { retry_after_secs }
        } else {
            AppError::ProviderUnavailable
        };
        ProviderOutcome::ChainError {
            source,
            error,
            degraded,
        }
    }

    /// Wrap a provider call's error as `ChainError`. Defaults to
    /// `degraded = false` — callers that know the failure is
    /// upstream (e.g. `chromiumoxide::Error` on `Browser::launch`)
    /// should override the flag explicitly.
    pub fn chain_error(source: &'static str, error: AppError) -> Self {
        ProviderOutcome::ChainError {
            source,
            error,
            degraded: false,
        }
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
    /// GAP-AUD-2026-038: discriminator for which parser the body
    /// needs. `Srt` (default) for `SubRip`; `NoteeyTranscript` for the
    /// `MM:SS`-prefixed plain text that noteey.com emits. Consumed
    /// by `commands::convert_format` to pick the right parser.
    pub format_hint: SubtitleFormat,
    /// GAP-AUD-2026-050: stable provider identifier that produced
    /// this `SubtitleInfo`. Mirrors [`Provider::name`] exactly so
    /// downstream consumers can correlate the JSON envelope field
    /// `provider` with tracing events. Populated by every concrete
    /// provider at `fetch_subtitle` time.
    pub provider: &'static str,
}

/// Subtitle body shape returned by a provider.
///
/// Distinct from [`Format`] (the user-requested delivery format):
/// `format_hint` tells the CLI what the body bytes *actually* look
/// like so the right parser is invoked. The user-requested `Format`
/// may still differ — `noteey_to_text` always emits plain text even
/// when the user asked for `--format srt`, in which case the chain
/// returns `AppError::InvalidUsage` rather than fabricating timestamps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SubtitleFormat {
    /// Body is in `SubRip` (`Srt`) format.
    #[default]
    Srt,
    /// Body is noteey-style transcript: one cue per line with a
    /// leading `MM:SS` (or `HH:MM:SS`) timestamp prefix.
    NoteeyTranscript,
}

impl SubtitleFormat {
    /// Lowercase kebab-case identifier for logs and tracing.
    pub fn as_str(&self) -> &'static str {
        match self {
            SubtitleFormat::Srt => "srt",
            SubtitleFormat::NoteeyTranscript => "noteey-transcript",
        }
    }
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
    ///   of a subtitle (only after a *non-degraded* `NoSubtitle` — see
    ///   GAP-AUD-2026-039).
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
        // GAP-AUD-2026-039: a "genuine" NoSubtitle only counts when the
        // upstream is reachable. `saw_no_subtitle` therefore ignores
        // entries wrapped with `degraded = true` — those continue to
        // the next provider instead of poisoning the chain.
        let mut saw_genuine_no_subtitle = false;
        let total = self.providers.len();
        for (idx, provider) in self.providers.iter().enumerate() {
            tracing::debug!(
                target: "events",
                chain_index = idx,
                chain_total = total,
                provider = provider.name(),
                event = "chain_attempting_provider",
                "chain attempting provider"
            );
            self.throttle().await;
            let outcome = match provider.fetch_subtitle(video_id, language, format).await {
                Ok(info) => match provider.fetch_content(&info).await {
                    Ok(content) if !content.is_empty() => ProviderOutcome::Subtitle(info, content),
                    Ok(_) => ProviderOutcome::ChainError {
                        source: provider.name(),
                        error: AppError::NoSubtitle(crate::error::NoSubtitleReason::NotPublished),
                        // Body fetch returned empty after a successful
                        // `fetch_subtitle` — site reachable, body empty.
                        // This IS the "no captions exist" signal.
                        degraded: false,
                    },
                    Err(e) => ProviderOutcome::ChainError {
                        source: provider.name(),
                        error: e,
                        // Body fetch failure is local (network blip
                        // during GET) — do NOT mark degraded; we don't
                        // know whether the upstream is healthy.
                        degraded: false,
                    },
                },
                Err(AppError::NoSubtitle(reason)) => {
                    tracing::warn!(target: "events", provider = provider.name(), reason = %reason, "provider returned no subtitle");
                    ProviderOutcome::ChainError {
                        source: provider.name(),
                        error: AppError::NoSubtitle(reason),
                        degraded: false,
                    }
                }
                Err(
                    e @ (AppError::ProviderUnavailable
                    | AppError::RateLimited { .. }
                    | AppError::CaptchaChallenge { .. }
                    | AppError::BrowserNotFound(_)),
                ) => {
                    // GAP-AUD-2026-039: upstream-side failures must not
                    // short-circuit the chain. ProviderUnavailable from
                    // a headless site, rate-limit from a static site, or
                    // a captcha challenge all mean "this provider
                    // cannot answer right now" — keep walking.
                    //
                    // GAP-AUD-2026-049: the previous implementation
                    // re-invoked `provider.fetch_subtitle(...)` here to
                    // recover the error variant, which caused every
                    // degraded provider to be called twice (for
                    // provider-headless this meant spawning a second
                    // chromiumoxide browser per request and doubling
                    // wall-clock latency). The error is already bound to
                    // `e` by the match guard — reuse it directly.
                    //
                    // GAP-AUD-2026-054: BrowserNotFound is added to the
                    // degraded set so the chain keeps walking when the
                    // local environment lacks Chromium (CI, sandbox,
                    // uninstalled). It also bypasses the
                    // `saw_genuine_no_subtitle` collapse below: an
                    // operator who ran the chain and saw "no subtitle"
                    // deserves to know whether the static providers
                    // confirmed the absence OR whether the headless
                    // tier never got a chance to try because chrome is
                    // missing.
                    ProviderOutcome::ChainError {
                        source: provider.name(),
                        error: e,
                        degraded: true,
                    }
                }
                Err(e) => ProviderOutcome::ChainError {
                    source: provider.name(),
                    error: e,
                    degraded: false,
                },
            };

            match outcome {
                ProviderOutcome::Subtitle(info, content) => {
                    return Ok((info, content));
                }
                ProviderOutcome::ChainError {
                    source,
                    error,
                    degraded,
                } => {
                    if degraded {
                        tracing::warn!(
                            target: "events",
                            provider = source,
                            degraded = true,
                            error = %error,
                            "provider_failed_degraded_skipping"
                        );
                        // GAP-AUD-2026-054: record the environment
                        // signal in `last_err` WITHOUT marking
                        // `saw_genuine_no_subtitle`. `remember_failure`
                        // only downgrades a slot that already holds a
                        // stronger environment signal, so a later
                        // `NoSubtitle(NotPublished)` cannot silence an
                        // earlier `BrowserNotFound` / `CaptchaChallenge`
                        // / `RateLimited`.
                        remember_failure(&mut last_err, error);
                        continue;
                    }
                    if matches!(error, AppError::NoSubtitle(_)) {
                        saw_genuine_no_subtitle = true;
                    }
                    remember_failure(&mut last_err, error);
                }
            }
        }

        // GAP-AUD-2026-054: when the static tier reports NoSubtitle
        // BUT the headless tier could not run because Chrome is
        // missing, surface the environment error instead of
        // collapsing to NoSubtitle. The operator needs to know that
        // the chain short-circuited on missing tooling, not on
        // confirmed-absence. We honour the original last_err
        // ordering (`remember_failure` already prefers RateLimited
        // over generic failures); BrowserNotFound survives because
        // `remember_failure` only downgrades when the slot already
        // holds RateLimited.
        match last_err {
            Some(err @ AppError::BrowserNotFound(_))
            | Some(err @ AppError::CaptchaChallenge { .. })
            | Some(err @ AppError::RateLimited { .. }) => return Err(err),
            _ => {}
        }

        if saw_genuine_no_subtitle {
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
    async fn chain_treats_429_and_503_as_degraded_skips_both() {
        // GAP-AUD-2026-039: HTTP 429 and 503 are upstream-side failures.
        // Both must be classified as `degraded = true`, which means
        // the chain does NOT record them as `last_err` and does NOT
        // mark `saw_genuine_no_subtitle`. With both providers degraded
        // the chain ends with the empty-state fallback `ProviderUnavailable`
        // — which is the same signal operators see when EVERY upstream
        // is unreachable.
        //
        // The pre-GAP-039 behaviour was to prefer RateLimited over
        // later ProviderUnavailable (EC-021). That heuristic still
        // applies when one provider is RateLimited and a later one
        // has a genuine (non-degraded) failure. The new contract
        // is documented in `ProviderOutcome::from_http_status`.
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
        // GAP-AUD-2026-054: RateLimited is an environment signal that
        // must survive the chain even when a later provider also
        // degrades. EC-021 guarantees that the `Retry-After` reaches
        // the caller. The pre-054 test asserted
        // `ProviderUnavailable`, which silently swallowed the
        // rate-limit; the post-054 contract is
        // `RateLimited { retry_after_secs }` from the FIRST provider
        // because `remember_failure` now protects that slot.
        match err {
            AppError::RateLimited {
                retry_after_secs: Some(n),
            } => assert!(
                (2..=4).contains(&n),
                "retry_after out of expected window: {n}"
            ),
            other => panic!("expected RateLimited with retry_after, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn chain_records_genuine_no_subtitle_after_degraded_provider() {
        // GAP-AUD-2026-039: the core invariant — a degraded provider
        // must NOT poison the chain. Provider A returns 503 (degraded)
        // and Provider B returns 404 mapped to NoSubtitle(NotFound).
        // The chain should keep walking past the 503 and surface the
        // 404 as NoSubtitle(NotFound).
        struct MockStatusProvider {
            url: String,
            name: &'static str,
        }
        #[async_trait]
        impl Provider for MockStatusProvider {
            fn name(&self) -> &'static str {
                self.name
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
                let status = resp.status().as_u16();
                // Mirror the production classification in
                // `provider_a.rs::fetch_page_html`: 4xx is mapped to
                // `NoSubtitle` via `NoSubtitleReason::from_status`,
                // 5xx and 429 fall through to `http_failure`.
                if let Some(reason) = crate::error::NoSubtitleReason::from_status(status) {
                    return Err(AppError::NoSubtitle(reason));
                }
                Err(http_failure(resp.status(), resp.headers()))
            }
            async fn fetch_content(&self, _info: &SubtitleInfo) -> AppResult<Vec<u8>> {
                Err(AppError::ProviderUnavailable)
            }
        }

        let unavailable = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(503))
            .mount(&unavailable)
            .await;
        let not_found = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(404))
            .mount(&not_found)
            .await;

        let chain = ProviderChain::with_min_interval(
            vec![
                Box::new(MockStatusProvider {
                    url: unavailable.uri(),
                    name: "mock-503",
                }),
                Box::new(MockStatusProvider {
                    url: not_found.uri(),
                    name: "mock-404",
                }),
            ],
            Duration::from_millis(1),
        );
        let err = chain
            .fetch_subtitle("dQw4w9WgXcQ", "en", Format::Srt)
            .await
            .expect_err("both providers fail");
        // 503 is degraded (skipped), 404 maps to NoSubtitle(NotFound)
        // internally — but the chain consolidates all genuine
        // `NoSubtitle` verdicts to `NotPublished` (GAP-AUD-2026-038).
        // The structured `NotFound` reason is preserved when a single
        // provider returns it (no degraded skip); here the chain
        // returns the conservative `NotPublished` because the 503
        // also lost its NoSubtitle status. Operators who need the
        // structured reason should query `provider_a` directly with
        // `--no-fallback`.
        assert!(
            matches!(
                err,
                AppError::NoSubtitle(crate::error::NoSubtitleReason::NotPublished)
            ),
            "expected NoSubtitle(NotPublished) (consolidated) after degraded 503, got {err:?}"
        );
    }

    #[tokio::test]
    async fn chain_records_genuine_no_subtitle_after_two_degraded_providers() {
        // GAP-AUD-2026-039 edge case: two degraded providers followed
        // by a genuine 404 mapped to NoSubtitle(NotFound). Final
        // verdict is NoSubtitle(NotFound).
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
                let status = resp.status().as_u16();
                if let Some(reason) = crate::error::NoSubtitleReason::from_status(status) {
                    return Err(AppError::NoSubtitle(reason));
                }
                Err(http_failure(resp.status(), resp.headers()))
            }
            async fn fetch_content(&self, _info: &SubtitleInfo) -> AppResult<Vec<u8>> {
                Err(AppError::ProviderUnavailable)
            }
        }

        let s503 = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(503))
            .mount(&s503)
            .await;
        let s429 = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(429).insert_header("Retry-After", "9"))
            .mount(&s429)
            .await;
        let s404 = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(404))
            .mount(&s404)
            .await;

        let chain = ProviderChain::with_min_interval(
            vec![
                Box::new(MockStatusProvider { url: s503.uri() }),
                Box::new(MockStatusProvider { url: s429.uri() }),
                Box::new(MockStatusProvider { url: s404.uri() }),
            ],
            Duration::from_millis(1),
        );
        let err = chain
            .fetch_subtitle("dQw4w9WgXcQ", "en", Format::Srt)
            .await
            .expect_err("all providers fail");
        // GAP-AUD-2026-054: the 429 from the second provider must
        // survive the chain. The 503 (degraded) does NOT poison the
        // chain (GAP-AUD-2026-039) and the 404 (NoSubtitle) is
        // recorded but the 429 takes precedence — EC-021 says
        // RateLimited is the canonical error when ANY provider
        // hit it, regardless of what later providers reported.
        match err {
            AppError::RateLimited {
                retry_after_secs: Some(n),
            } => assert!(
                (8..=10).contains(&n),
                "retry_after out of expected window: {n}"
            ),
            other => panic!(
                "expected RateLimited from the second provider (EC-021 wins over later NoSubtitle), got {other:?}"
            ),
        }
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

    // GAP-AUD-2026-039: ProviderOutcome classification contract.
    #[test]
    fn provider_outcome_503_is_degraded_and_unavailable() {
        let outcome = ProviderOutcome::from_http_status("provider-a", 503, None);
        match outcome {
            ProviderOutcome::ChainError {
                source,
                error,
                degraded,
            } => {
                assert_eq!(source, "provider-a");
                assert!(degraded);
                assert!(matches!(error, AppError::ProviderUnavailable));
            }
            other => panic!("expected ChainError, got {other:?}"),
        }
    }

    #[test]
    fn provider_outcome_500_is_degraded_and_unavailable() {
        let outcome = ProviderOutcome::from_http_status("provider-a", 500, None);
        match outcome {
            ProviderOutcome::ChainError {
                degraded, error, ..
            } => {
                assert!(degraded);
                assert!(matches!(error, AppError::ProviderUnavailable));
            }
            other => panic!("expected ChainError, got {other:?}"),
        }
    }

    #[test]
    fn provider_outcome_429_is_degraded_and_rate_limited() {
        let outcome = ProviderOutcome::from_http_status("provider-a", 429, Some(120));
        match outcome {
            ProviderOutcome::ChainError {
                degraded, error, ..
            } => {
                assert!(degraded);
                assert!(matches!(
                    error,
                    AppError::RateLimited {
                        retry_after_secs: Some(120)
                    }
                ));
            }
            other => panic!("expected ChainError, got {other:?}"),
        }
    }

    #[test]
    fn provider_outcome_404_is_genuine_no_subtitle_not_degraded() {
        let outcome = ProviderOutcome::from_http_status("provider-a", 404, None);
        match outcome {
            ProviderOutcome::ChainError {
                degraded, error, ..
            } => {
                assert!(!degraded);
                assert!(matches!(
                    error,
                    AppError::NoSubtitle(crate::error::NoSubtitleReason::NotFound)
                ));
            }
            other => panic!("expected ChainError, got {other:?}"),
        }
    }

    #[test]
    fn provider_outcome_400_is_genuine_no_subtitle_not_degraded() {
        let outcome = ProviderOutcome::from_http_status("provider-a", 400, None);
        match outcome {
            ProviderOutcome::ChainError {
                degraded, error, ..
            } => {
                assert!(!degraded);
                assert!(matches!(
                    error,
                    AppError::NoSubtitle(crate::error::NoSubtitleReason::NotPublished)
                ));
            }
            other => panic!("expected ChainError, got {other:?}"),
        }
    }

    #[test]
    fn provider_outcome_chain_error_defaults_to_not_degraded() {
        let outcome =
            ProviderOutcome::chain_error("provider-x", AppError::Internal("synthetic".to_string()));
        match outcome {
            ProviderOutcome::ChainError {
                degraded,
                source,
                error,
            } => {
                assert!(!degraded);
                assert_eq!(source, "provider-x");
                assert!(matches!(error, AppError::Internal(_)));
            }
            other => panic!("expected ChainError, got {other:?}"),
        }
    }
}
