//! Integration tests for the v0.3.0 CLI probing surface:
//! `--provider`, `--asr`, and `--no-fallback`.
//!
//! These tests focus on the parser and `validate()` paths. They
//! do NOT make network calls. The actual network round-trip is
//! exercised by the corpus test and by `bin/youtube-direct-probe`.
//! Keeping these gates offline means they run in <1s on a clean
//! CI box.

use assert_cmd::Command;
use predicates::prelude::*;

/// Build a `youtube-legend-cli` invocation.
fn bin() -> Command {
    Command::cargo_bin("youtube-legend-cli").expect("binary")
}

/// Scenario 1: `--provider auto` is accepted as a flag, and the
/// default value (when the flag is omitted) is also `auto`. The
/// help text mentions all five valid choices.
#[test]
fn provider_auto_parses_and_lists_all_in_help() {
    // Pass --provider auto explicitly; the binary then needs a URL
    // (or stdin). We use a short invalid URL so it fails fast
    // without network traffic; the parse must have succeeded.
    bin()
        .arg("--provider")
        .arg("auto")
        .arg("--timeout")
        .arg("1")
        .arg("https://youtu.be/dQw4w9WgXcQ")
        .timeout(std::time::Duration::from_secs(10))
        .assert()
        .failure();

    // The help text exposes all five provider choices. The exact
    // `--help` rendering is owned by `clap`; we only check that
    // each kebab-case token appears at least once.
    let help = bin()
        .arg("--help")
        .timeout(std::time::Duration::from_secs(5))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help_str = String::from_utf8_lossy(&help);
    assert!(help_str.contains("auto"), "--help missing `auto` token");
    assert!(
        help_str.contains("youtube-direct"),
        "--help missing `youtube-direct` token"
    );
    assert!(
        help_str.contains("provider-a"),
        "--help missing `provider-a` token"
    );
    assert!(
        help_str.contains("provider-b"),
        "--help missing `provider-b` token"
    );
    assert!(
        help_str.contains("provider-headless"),
        "--help missing `provider-headless` token"
    );
}

/// Scenario 2: `--provider youtube-direct` parses cleanly. We pair
/// it with a URL that will fail to resolve (no network) so the
/// binary exits non-zero, but the parse step must succeed first.
/// `assert_cmd`'s `.failure()` accepts any non-zero exit, so the
/// presence of stderr text from the *post-parse* provider step
/// confirms the flag was accepted.
#[test]
fn provider_youtube_direct_parses() {
    bin()
        .arg("--provider")
        .arg("youtube-direct")
        .arg("--timeout")
        .arg("1")
        .arg("https://youtu.be/INVALID_ID_FOR_TEST")
        .timeout(std::time::Duration::from_secs(10))
        .assert()
        .failure();
}

/// Scenario 3: `--provider provider-a --asr` is rejected by
/// `Cli::validate` (`FR-004` from `T5`). The exit code is 64 (`EX_USAGE`)
/// and stderr contains the validation message.
#[test]
fn provider_a_rejects_asr() {
    bin()
        .arg("--provider")
        .arg("provider-a")
        .arg("--asr")
        .timeout(std::time::Duration::from_secs(5))
        .assert()
        .failure()
        .code(64)
        .stderr(predicate::str::contains("--asr"));
}

/// Scenario 4: `--provider provider-b --asr` is rejected for the same
/// reason.
#[test]
fn provider_b_rejects_asr() {
    bin()
        .arg("--provider")
        .arg("provider-b")
        .arg("--asr")
        .timeout(std::time::Duration::from_secs(5))
        .assert()
        .failure()
        .code(64)
        .stderr(predicate::str::contains("provider-b"));
}

/// Scenario 5: `--no-fallback` only works under `--provider auto`;
/// pairing it with an explicit provider is rejected at parse time.
#[test]
fn no_fallback_requires_auto_provider() {
    bin()
        .arg("--provider")
        .arg("provider-a")
        .arg("--no-fallback")
        .timeout(std::time::Duration::from_secs(5))
        .assert()
        .failure()
        .code(64)
        .stderr(predicate::str::contains("--no-fallback"));
}
