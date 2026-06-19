//! Integration tests for the headless-browser provider's offline-friendly
//! surface: env-var short-circuit, `name()` return value, and the
//! `BrowserNotFound` error contract when no browser is available.
//!
//! We deliberately avoid `ProviderHeadless::fetch_subtitle` against a
//! real Chromium — the `#[ignore]`'d `headless_downloads_real_subtitle`
//! unit test in `src/provider/provider_headless.rs` covers the live path.

#![cfg(feature = "headless")]

use serial_test::serial;
use std::env;
use youtube_legend_cli::error::AppError;
use youtube_legend_cli::provider::provider_headless::ProviderHeadless;
use youtube_legend_cli::provider::{Format, Provider};

#[test]
fn provider_headless_name_is_headless() {
    let p = ProviderHeadless::new();
    assert_eq!(p.name(), "provider-headless");
}

#[test]
fn builder_with_language_is_chainable() {
    // Verify the builder method compiles and returns a usable value.
    // The internal `language` field is private, so we only assert
    // that the chain itself does not panic.
    let _p = ProviderHeadless::new().with_language("pt");
}

#[tokio::test]
#[serial]
async fn yt_legend_no_network_env_blocks_fetch_subtitle() {
    // serial_test::serial guarantees we own the env var for the
    // duration of the test. Setting and removing here is safe.
    let prev = env::var("YT_LEGEND_NO_NETWORK").ok();
    env::set_var("YT_LEGEND_NO_NETWORK", "1");
    let p = ProviderHeadless::new();
    let err = p
        .fetch_subtitle("dQw4w9WgXcQ", "en", Format::Srt)
        .await
        .expect_err("YT_LEGEND_NO_NETWORK must block fetch_subtitle");
    match prev {
        Some(v) => env::set_var("YT_LEGEND_NO_NETWORK", v),
        None => env::remove_var("YT_LEGEND_NO_NETWORK"),
    }
    assert!(
        matches!(err, AppError::ProviderUnavailable),
        "expected ProviderUnavailable, got {err:?}"
    );
}

#[tokio::test]
#[serial]
async fn yt_legend_no_network_env_blocks_fetch_subtitle_with_any_value() {
    // The env var's value does not matter; only its presence is the
    // switch. Verify with an unusual value to lock in the contract.
    let prev = env::var("YT_LEGEND_NO_NETWORK").ok();
    env::set_var("YT_LEGEND_NO_NETWORK", "arbitrary-string");
    let p = ProviderHeadless::new();
    let err = p
        .fetch_subtitle("dQw4w9WgXcQ", "pt", Format::Txt)
        .await
        .expect_err("YT_LEGEND_NO_NETWORK must block fetch_subtitle");
    match prev {
        Some(v) => env::set_var("YT_LEGEND_NO_NETWORK", v),
        None => env::remove_var("YT_LEGEND_NO_NETWORK"),
    }
    assert!(
        matches!(err, AppError::ProviderUnavailable),
        "expected ProviderUnavailable, got {err:?}"
    );
}
