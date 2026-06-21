//! Wiremock integration tests for provider A's error-mapping surface.
//!
//! These tests validate that the public `AppError` enum correctly
//! classifies the HTTP statuses a real provider must produce. The
//! `Provider` trait requires constructing a `SubtitleInfo` (which is
//! `#[non_exhaustive]` and therefore unconstructible from outside
//! the crate), so we exercise the classifier indirectly: a tiny
//! helper returns a `Result<SubtitleInfo, AppError>`-shaped error by
//! applying the same logic the real `ProviderA::fetch_subtitle` does
//! in `src/provider/provider_a.rs`.
//!
//! Five canonical scenarios (success, 400, 404, 429, 500) cover the
//! most common integration failure modes; all run in <1s combined.

use std::time::Duration;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};
use youtube_legend_cli::error::{AppError, NoSubtitleReason};

/// Translate an HTTP response into the matching `AppError` variant,
/// mirroring the mapping in `ProviderA::fetch_subtitle`. The success
/// path returns `Ok(())` so the assertion can branch on the result
/// without needing a `SubtitleInfo` constructor.
async fn classify_response(url: &str) -> Result<(), AppError> {
    let resp = reqwest::Client::new()
        .get(url)
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .map_err(AppError::Http)?;
    let status = resp.status();
    if status.as_u16() == 429 {
        let retry = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());
        return Err(AppError::RateLimited {
            retry_after_secs: retry,
        });
    }
    if let Some(reason) = NoSubtitleReason::from_status(status.as_u16()) {
        return Err(AppError::NoSubtitle(reason));
    }
    if !status.is_success() {
        return Err(AppError::ProviderUnavailable);
    }
    Ok(())
}

#[tokio::test]
async fn provider_a_fetches_subtitle_success() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>ok</html>"))
        .mount(&server)
        .await;
    classify_response(&server.uri())
        .await
        .expect("200 must succeed");
}

#[tokio::test]
async fn provider_a_returns_no_subtitle_on_400() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(400))
        .mount(&server)
        .await;
    let err = classify_response(&server.uri())
        .await
        .expect_err("400 must fail");
    // GAP-E2E-026: HTTP 400 from the YouTube timedtext endpoint means
    // "no captions exist for this video". The previous mapping sent
    // 400 to ProviderUnavailable (exit 69); the new mapping unifies
    // 400 with the rest of the NoSubtitle family (exit 66). The
    // helper's `from_status(400)` now resolves to NotPublished.
    assert!(matches!(
        err,
        AppError::NoSubtitle(NoSubtitleReason::NotPublished)
    ));
}

#[tokio::test]
async fn provider_a_returns_no_subtitle_on_404() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let err = classify_response(&server.uri())
        .await
        .expect_err("404 must fail");
    assert!(matches!(
        err,
        AppError::NoSubtitle(NoSubtitleReason::NotFound)
    ));
}

#[tokio::test]
async fn provider_a_returns_rate_limited_on_429_with_retry_after() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "5"))
        .mount(&server)
        .await;
    let err = classify_response(&server.uri())
        .await
        .expect_err("429 must fail");
    assert!(matches!(
        err,
        AppError::RateLimited {
            retry_after_secs: Some(5)
        }
    ));
}

#[tokio::test]
async fn provider_a_returns_provider_unavailable_on_500() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let err = classify_response(&server.uri())
        .await
        .expect_err("500 must fail");
    assert!(matches!(err, AppError::ProviderUnavailable));
}
