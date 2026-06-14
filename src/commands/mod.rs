//! Top-level command dispatch: extract one URL or batch many.

use crate::cli::Cli;
use crate::error::{AppError, AppResult};
use crate::parse::srt_to_text;
use crate::parse::video_id::extract_video_id;
use crate::provider::{Format, ProviderChain};
use crate::text::normalize_nfc;
use serde::Serialize;
use std::process::ExitCode;

pub mod batch;
pub mod extract;

#[derive(Debug, Serialize)]
struct JsonSuccess {
    video_id: String,
    language: String,
    format: String,
    content: String,
    bytes: u64,
    duration_ms: u64,
    source: String,
}

#[derive(Debug, Serialize)]
struct JsonError {
    error: bool,
    code: u8,
    message: String,
}

/// Dispatch a validated [`Cli`] to the single-URL or batch runner.
///
/// # Errors
///
/// - [`AppError::InvalidUsage`] when the [`Cli::validate`] check fails.
/// - All provider / network / cache / IO errors bubble up.
#[tracing::instrument(level = "debug", err, skip(cli), fields(batch = cli.batch, url = ?cli.url, json = cli.json, verbose = cli.verbose))]
pub async fn run(cli: Cli) -> AppResult<ExitCode> {
    if let Err(msg) = cli.validate() {
        return Err(AppError::InvalidUsage(msg));
    }

    let chain = build_provider_chain(&cli);

    if cli.batch {
        batch::run(&cli, &chain).await
    } else {
        extract::run(&cli, &chain).await
    }
}

fn build_provider_chain(cli: &Cli) -> ProviderChain {
    use crate::provider::provider_a::ProviderA;
    use crate::provider::provider_b::ProviderB;

    let user_agent = cli.effective_user_agent();
    let mut providers: Vec<Box<dyn crate::provider::Provider>> = Vec::new();
    if let Ok(p) = ProviderA::with_user_agent(&user_agent) {
        providers.push(Box::new(p));
    }
    if let Ok(p) = ProviderB::with_user_agent(&user_agent) {
        providers.push(Box::new(p));
    }
    #[cfg(feature = "headless")]
    {
        let hl = crate::provider::provider_headless::ProviderHeadless::new()
            .with_language(language_to_str(cli.lang));
        providers.push(Box::new(hl));
    }
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
pub fn convert_format(content: &[u8], format: Format) -> AppResult<String> {
    match format {
        Format::Srt => String::from_utf8(content.to_vec())
            .map_err(|e| AppError::Internal(format!("srt is not valid utf-8: {e}"))),
        Format::Txt => {
            let srt_text = String::from_utf8(content.to_vec())
                .map_err(|e| AppError::Internal(format!("srt is not valid utf-8: {e}")))?;
            srt_to_text(&srt_text)
        }
    }
}

/// Write the success envelope to stdout (text or JSON depending on
/// `--json`).
///
/// # Errors
///
/// - [`AppError::Serde`] when serialising the JSON envelope.
/// - [`AppError::Io`] on stdout write failure.
pub async fn output_success(
    cli: &Cli,
    video_id: &str,
    content: &str,
    source: &str,
    bytes: u64,
    duration_ms: u64,
) -> AppResult<()> {
    if cli.json {
        let payload = JsonSuccess {
            video_id: video_id.to_string(),
            language: language_to_str(cli.lang).to_string(),
            format: format_to_str(cli.format).to_string(),
            content: normalize_nfc(content),
            bytes,
            duration_ms,
            source: source.to_string(),
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
    if cli.verbose && !cli.quiet {
        let _ = crate::io::write_to_stderr(&format!("extracted video id: {id}\n"));
    }
    Ok(id)
}
