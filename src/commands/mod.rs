//! Top-level command dispatch: extract one URL or batch many.

use crate::cli::{Cli, ProviderChoice};
use crate::error::{AppError, AppResult};
use crate::parse::video_id::extract_video_id;
use crate::provider::{Format, ProviderChain};
use crate::text::normalize_nfc;
use serde::Serialize;
use std::process::ExitCode;

pub mod batch;
pub mod extract;

#[derive(Debug, Serialize)]
struct JsonSuccess {
    /// Provider name that delivered the transcript (`provider-noteey`
    /// or `cache` for cache hits).
    provider: &'static str,
    video_id: String,
    language: String,
    format: String,
    content: String,
    /// GAP-AUD-2026-050: renamed from `bytes` to `byte_size` to match
    /// the contract documented in `docs/AGENTS.pt-BR.md`.
    byte_size: u64,
    duration_ms: u64,
    /// GAP-AUD-2026-050: renamed from `source` to `source_url` to
    /// match the contract documented in `docs/AGENTS.pt-BR.md`.
    source_url: String,
}

#[derive(Debug, Serialize)]
struct JsonError {
    error: bool,
    code: u8,
    message: String,
}

/// GAP-E2E-009: dry-run envelope. Emitted to stdout under `--dry-run`
/// so operators can distinguish cache-miss-dry-run (would fetch)
/// from cache-hit-dry-run (would skip) by parsing the `event` field,
/// instead of branching on the legacy `exit 66` signal that the
/// previous implementation produced.
#[derive(Debug, Serialize)]
struct JsonDryRun {
    event: &'static str,
    video_id: String,
    language: String,
    format: String,
    would_fetch: bool,
}

/// Dispatch a validated [`Cli`] to the single-URL or batch runner.
///
/// # Errors
///
/// - [`AppError::InvalidUsage`] when the [`Cli::validate`] check fails.
/// - All provider / network / cache / IO errors bubble up.
#[tracing::instrument(level = "debug", err, skip(cli), fields(batch = cli.batch, url = ?cli.url, json = cli.json, verbose = cli.verbose, provider = ?cli.provider))]
pub async fn run(cli: Cli) -> AppResult<ExitCode> {
    // GAP-E2E-015: `cli.validate()` now returns `AppResult<()>` so the
    // `?` propagates `AppError::InvalidUsage` directly. The previous
    // bridge `if let Err(msg) = cli.validate() { return Err(AppError::InvalidUsage(msg)); }`
    // is gone.
    cli.validate()?;

    let chain = build_provider_chain(&cli);

    if cli.batch {
        batch::run(&cli, &chain).await
    } else {
        extract::run(&cli, &chain).await
    }
}

/// Build the [`ProviderChain`] for a given [`Cli`].
///
/// The CLI uses exclusively the noteey.com provider. Both `auto`
/// (default) and `provider-noteey` resolve to a chain containing only
/// [`crate::provider::provider_noteey::ProviderNoteey`].
#[tracing::instrument(level = "debug", skip(cli), fields(provider = ?cli.provider))]
fn build_provider_chain(cli: &Cli) -> ProviderChain {
    let mut providers: Vec<Box<dyn crate::provider::Provider>> = Vec::new();

    let _choice = cli.provider.unwrap_or(ProviderChoice::Auto);

    let noteey = crate::provider::provider_noteey::ProviderNoteey::new()
        .with_language(language_to_str(cli.lang));
    providers.push(Box::new(noteey));

    ProviderChain::new(providers)
}

/// Translate a CLI [`crate::cli::FormatArg`] into the provider-layer [`Format`].
pub fn format_to_provider_format(arg: crate::cli::FormatArg) -> Format {
    match arg {
        crate::cli::FormatArg::Txt => Format::Txt,
        crate::cli::FormatArg::Srt => Format::Srt,
    }
}

/// Translate a CLI [`crate::cli::LanguageArg`] into the ISO 639-1 code
/// string consumed by the provider layer.
pub fn language_to_str(arg: crate::cli::LanguageArg) -> &'static str {
    match arg {
        crate::cli::LanguageArg::En => "en",
        crate::cli::LanguageArg::Pt => "pt",
        crate::cli::LanguageArg::Es => "es",
        crate::cli::LanguageArg::Fr => "fr",
        crate::cli::LanguageArg::De => "de",
        crate::cli::LanguageArg::It => "it",
    }
}

/// Convert raw subtitle bytes into the user-requested text form.
///
/// # Errors
///
/// - [`AppError::Internal`] when the bytes are not valid UTF-8.
/// - [`AppError::InvalidInput`] / [`AppError::SubtitleTooLarge`] when
///   the SRT body is malformed or exceeds the 50 MiB cap.
/// - [`AppError::InvalidUsage`] when `--format srt` is requested but
///   the body is a noteey transcript (no SRT framing available).
pub fn convert_format(
    content: &[u8],
    format: Format,
    format_hint: crate::provider::SubtitleFormat,
) -> AppResult<String> {
    match (format, format_hint) {
        // SRT requested and the body is real SubRip — pass through.
        (Format::Srt, crate::provider::SubtitleFormat::Srt) => {
            String::from_utf8(content.to_vec())
                .map_err(|e| AppError::Internal(format!("srt is not valid utf-8: {e}")))
        }
        // Txt requested and the body is SRT — convert via srt_to_text.
        (Format::Txt, crate::provider::SubtitleFormat::Srt) => {
            let srt_text = String::from_utf8(content.to_vec())
                .map_err(|e| AppError::Internal(format!("srt is not valid utf-8: {e}")))?;
            crate::parse::srt_to_text(&srt_text)
        }
        // Txt requested and the body is noteey-style transcript.
        (Format::Txt, crate::provider::SubtitleFormat::NoteeyTranscript) => {
            let raw = String::from_utf8(content.to_vec())
                .map_err(|e| AppError::Internal(format!("noteey body not valid utf-8: {e}")))?;
            crate::parse::noteey_to_text(&raw)
        }
        // SRT requested but the body is noteey-style — reject rather
        // than fabricate SubRip timestamps (noteey does not carry
        // end-of-cue info to produce real SRT).
        (Format::Srt, crate::provider::SubtitleFormat::NoteeyTranscript) => Err(
            AppError::InvalidUsage(
                "--format srt is not available when the only source is noteey.com \
                 (transcript has no SRT framing); use --format txt (default)"
                    .to_string(),
            ),
        ),
    }
}

/// Write the success envelope to stdout (text or JSON depending on
/// `--json`).
///
/// # Errors
///
/// - [`AppError::Serde`] when serialising the JSON envelope.
/// - [`AppError::Io`] on stdout write failure.
///
/// # Envelope shape
///
/// JSON envelope fields (GAP-AUD-2026-050):
/// - `provider` — which provider delivered (`provider-noteey`,
///   `provider-headless`, `youtube-direct`, `provider-a`,
///   `provider-b`, or `cache` for cache hits).
/// - `video_id`, `language`, `format` — request inputs.
/// - `content` — the cleaned transcript text (utf-8 NFC).
/// - `byte_size` — length of `content` in bytes.
/// - `duration_ms` — wall-clock time for the fetch (or cache lookup).
/// - `source_url` — the upstream URL that returned the raw body
///   (noteey-prefixed for noteey, youtube timedtext for direct, etc).
pub async fn output_success(
    cli: &Cli,
    provider: &'static str,
    video_id: &str,
    content: &str,
    source_url: &str,
    byte_size: u64,
    duration_ms: u64,
) -> AppResult<()> {
    if cli.json {
        let payload = JsonSuccess {
            provider,
            video_id: video_id.to_string(),
            language: language_to_str(cli.lang).to_string(),
            format: format_to_str(cli.format).to_string(),
            content: normalize_nfc(content),
            byte_size,
            duration_ms,
            source_url: source_url.to_string(),
        };
        let json = serde_json::to_string(&payload).map_err(AppError::Serde)?;
        crate::io::write_subtitle_to_stdout(json.as_bytes()).await?;
    } else {
        let nfc = normalize_nfc(content);
        crate::io::write_subtitle_to_stdout(nfc.as_bytes()).await?;
    }
    Ok(())
}

/// Best-effort write of the error envelope to stdout when `--json` is
/// set. Errors here are intentionally swallowed: the user already sees
/// the error via the `tracing` / `Termination` path.
///
/// # Errors
///
/// Returns [`AppError::Io`], [`AppError::Serde`], or
/// [`AppError::Internal`] when the envelope cannot be serialised or
/// written to stdout. The error path itself is best-effort: callers
/// should not treat a return value from this function as fatal because
/// the original error has already been emitted via the regular
/// `Termination` flow.
pub async fn output_error(cli: &Cli, err: &AppError) -> AppResult<()> {
    if cli.json {
        let payload = JsonError {
            error: true,
            code: err.exit_code(),
            message: err.to_string(),
        };
        if let Ok(json) = serde_json::to_string(&payload) {
            let _ = crate::io::write_subtitle_to_stdout(json.as_bytes()).await;
        }
    }
    Ok(())
}

/// GAP-E2E-009: emit the dry-run envelope to stdout. When `--json`
/// is set the payload is a single JSON object per line; without
/// `--json` we emit a human-readable line so a curl-like inspection
/// still surfaces the signal. `would_fetch` reports whether the
/// dry-run encountered a cache miss (true) or hit (false), letting
/// callers branch without parsing the `event` string.
///
/// # Errors
///
/// - [`AppError::Serde`] when serialising the JSON envelope fails.
/// - [`AppError::Io`] when writing the envelope to stdout fails.
pub async fn output_dry_run(cli: &Cli, video_id: &str, would_fetch: bool) -> AppResult<()> {
    if cli.json {
        let payload = JsonDryRun {
            event: "dry_run_cache_miss",
            video_id: video_id.to_string(),
            language: language_to_str(cli.lang).to_string(),
            format: format_to_str(cli.format).to_string(),
            would_fetch,
        };
        if let Ok(json) = serde_json::to_string(&payload) {
            crate::io::write_subtitle_to_stdout(json.as_bytes()).await?;
        }
    } else if would_fetch {
        crate::io::write_subtitle_to_stdout(format!("dry_run_cache_miss {video_id}\n").as_bytes())
            .await?;
    } else {
        crate::io::write_subtitle_to_stdout(format!("dry_run_cache_hit {video_id}\n").as_bytes())
            .await?;
    }
    Ok(())
}

fn format_to_str(arg: crate::cli::FormatArg) -> &'static str {
    match arg {
        crate::cli::FormatArg::Txt => "txt",
        crate::cli::FormatArg::Srt => "srt",
    }
}

/// Resolve the URL to extract: the positional arg if present, else a
/// single line read from stdin.
///
/// # Errors
///
/// - [`AppError::InvalidUsage`] when `--batch` is set (the batch
///   runner reads stdin directly).
/// - All errors from [`crate::io::read_url_from_stdin`].
pub async fn extract_url_from_input(cli: &Cli) -> AppResult<String> {
    if let Some(url) = &cli.url {
        return Ok(url.clone());
    }
    if cli.batch {
        return Err(AppError::InvalidUsage(
            "extract cannot be called with --batch".to_string(),
        ));
    }
    crate::io::read_url_from_stdin().await
}

/// Extract the video id from `url` and emit a verbose-mode line to
/// stderr if `--verbose` is set.
///
/// # Errors
///
/// - Any error from [`extract_video_id`].
pub fn parse_video_id_from_url(cli: &Cli, url: &str) -> AppResult<String> {
    let id = extract_video_id(url)?;
    // GAP-E2E-017: route the verbose line through `tracing` instead of
    // `io::write_to_stderr` so the `tracing-subscriber` EnvFilter built
    // in `logging.rs` (which honours `--quiet` via `EnvFilter::new("error")`)
    // actually silences it. The previous direct call bypassed the filter
    // and made the `--quiet` flag inert for this path.
    if cli.verbose && !cli.quiet {
        tracing::info!(target: "events", event = "video_id_extracted", video_id = %id);
    }
    Ok(id)
}
