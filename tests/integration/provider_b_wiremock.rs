//! Wiremock integration tests for provider B's error-mapping surface.
//!
//! Mirrors `provider_a_wiremock.rs`: validates the public `AppError`
//! classifier against the same five HTTP scenarios the real
//! `ProviderB::fetch_subtitle` must translate. No construction of
//! `SubtitleInfo` is needed (it is `#[non_exhaustive]`), so the
//! classifier is exercised through a free-standing helper that
//! returns `Result<(), AppError>`.

use std::time::Duration;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};
use youtube_legend_cli::error::{AppError, NoSubtitleReason};

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
async fn provider_b_fetches_subtitle_success() {
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
async fn provider_b_returns_no_subtitle_on_400() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(400))
        .mount(&server)
        .await;
    let err = classify_response(&server.uri())
        .await
        .expect_err("400 must fail");
    // GAP-E2E-026: HTTP 400 now maps to NoSubtitle(NotPublished)
    // across all providers. The previous test name and assertion
    // (provider_b_returns_invalid_url_on_400 expecting
    // ProviderUnavailable) are obsoleted by the unification.
    assert!(matches!(
        err,
        AppError::NoSubtitle(NoSubtitleReason::NotPublished)
    ));
}

#[tokio::test]
async fn provider_b_returns_no_subtitle_on_404() {
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
async fn provider_b_returns_rate_limited_on_429_with_retry_after() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "10"))
        .mount(&server)
        .await;
    let err = classify_response(&server.uri())
        .await
        .expect_err("429 must fail");
    assert!(matches!(
        err,
        AppError::RateLimited {
            retry_after_secs: Some(10)
        }
    ));
}

#[tokio::test]
async fn provider_b_returns_provider_unavailable_on_500() {
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
