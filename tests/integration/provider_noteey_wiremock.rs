//! Integration tests for the noteey headless-browser provider's
//! offline-friendly surface: env-var short-circuit, `name()` return
//! value, builder chaining, and `SubtitleFormat::NoteeyTranscript`
//! propagation.
//!
//! GAP-AUD-2026-038: this provider is the auto-fallback when
//! downsub.com is degraded. We deliberately avoid
//! `ProviderNoteey::fetch_subtitle` against a real Chromium — the
//! live path is exercised in `src/provider/provider_noteey.rs` via a
//! `#[ignore]`'d test that requires network access.

use serial_test::serial;
use std::env;
use youtube_legend_cli::error::AppError;
use youtube_legend_cli::provider::provider_noteey::ProviderNoteey;
use youtube_legend_cli::provider::{Format, Provider, SubtitleFormat};

#[test]
fn provider_noteey_name_is_noteey() {
    let p = ProviderNoteey::new();
    assert_eq!(p.name(), "provider-noteey");
}

#[test]
fn builder_with_language_is_chainable() {
    // Verify the builder method compiles and returns a usable value.
    // The internal `language` field is private, so we only assert
    // that the chain itself does not panic.
    let _p = ProviderNoteey::new().with_language("pt-BR");
}

#[tokio::test]
#[serial]
async fn yt_legend_no_network_env_blocks_fetch_subtitle() {
    // serial_test::serial guarantees we own the env var for the
    // duration of the test. Setting and removing here is safe.
    let prev = env::var("YT_LEGEND_NO_NETWORK").ok();
    env::set_var("YT_LEGEND_NO_NETWORK", "1");
    let p = ProviderNoteey::new();
    let err = p
        .fetch_subtitle("dQw4w9WgXcQ", "en", Format::Txt)
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
    let p = ProviderNoteey::new();
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

#[test]
fn noteey_format_hint_distinguishes_from_srt() {
    // GAP-AUD-2026-038: the discriminator enum is what lets
    // `convert_format` pick the right parser. This test locks in
    // the contract that the two variants are not equal and both
    // serialise to the expected lowercase identifiers.
    assert_ne!(SubtitleFormat::Srt, SubtitleFormat::NoteeyTranscript);
    assert_eq!(SubtitleFormat::Srt.as_str(), "srt");
    assert_eq!(
        SubtitleFormat::NoteeyTranscript.as_str(),
        "noteey-transcript"
    );
}
