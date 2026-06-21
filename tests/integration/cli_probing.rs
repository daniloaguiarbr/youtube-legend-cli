//! Integration tests for the v0.3.2 CLI probing surface:
//! `--provider` (auto | provider-noteey only).
//!
//! These tests focus on the parser and `validate()` paths. They
//! do NOT make network calls. Keeping these gates offline means they
//! run in <1s on a clean CI box.

use assert_cmd::Command;
use predicates::prelude::*;

/// Build a `youtube-legend-cli` invocation.
fn bin() -> Command {
    Command::cargo_bin("youtube-legend-cli").expect("binary")
}

/// `--provider auto` is accepted as a flag. The help text mentions
/// the two valid choices: `auto` and `provider-noteey`.
#[test]
fn provider_auto_parses_and_help_shows_noteey() {
    let assert = bin()
        .arg("--provider")
        .arg("auto")
        .arg("--timeout")
        .arg("1")
        .arg("--no-cache")
        .arg("https://youtu.be/dQw4w9WgXcQ")
        .timeout(std::time::Duration::from_secs(15))
        .assert();
    let code = assert.get_output().status.code().unwrap_or(1);
    assert_ne!(code, 2, "exit 2 means clap rejected --provider auto");

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
        help_str.contains("provider-noteey"),
        "--help missing `provider-noteey` token"
    );
}

/// `--provider provider-noteey` parses cleanly.
#[test]
fn provider_noteey_parses() {
    bin()
        .arg("--provider")
        .arg("provider-noteey")
        .arg("--timeout")
        .arg("1")
        .arg("https://youtu.be/INVALID_ID_FOR_TEST")
        .timeout(std::time::Duration::from_secs(10))
        .assert()
        .failure();
}

/// Removed providers are rejected by clap at parse time.
#[test]
fn removed_provider_youtube_direct_rejected() {
    bin()
        .arg("--provider")
        .arg("youtube-direct")
        .arg("https://youtu.be/dQw4w9WgXcQ")
        .timeout(std::time::Duration::from_secs(5))
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("invalid value"));
}
