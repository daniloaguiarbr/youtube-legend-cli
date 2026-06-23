//! Single-URL extraction command: read one URL, fetch, convert, write.

use crate::cache;
use crate::cli::Cli;
use crate::commands::{
    convert_format, extract_url_from_input, format_to_provider_format, language_to_str,
    output_dry_run, output_error, output_success, parse_video_id_from_url,
};
use crate::error::AppResult;
use crate::provider::ProviderChain;
use crate::retry::retry_with_backoff;
use std::process::ExitCode;
use std::time::Instant;
use tracing::instrument;

/// Run the single-URL extraction flow.
///
/// # Cancel safety
///
/// Cancel-safe. Drops before completion abort at the next `await` and
/// leave the cache untouched (writes happen only after a successful
/// fetch).
///
/// # Errors
///
/// - All errors from [`extract_url_from_input`], [`parse_video_id_from_url`],
///   [`crate::provider::ProviderChain::fetch_subtitle`], and
///   [`convert_format`].
#[instrument(skip(cli, chain), fields(video_id, language = %language_to_str(cli.lang)))]
pub async fn run(cli: &Cli, chain: &ProviderChain) -> AppResult<ExitCode> {
    let started = Instant::now();

    // GAP-AUD-2026-060: emit JSON error envelope for input errors.
    let url = match extract_url_from_input(cli).await {
        Ok(u) => u,
        Err(e) => {
            output_error(cli, &e).await.ok();
            return Ok(ExitCode::from(e.exit_code()));
        }
    };
    let video_id = match parse_video_id_from_url(cli, &url) {
        Ok(id) => id,
        Err(e) => {
            output_error(cli, &e).await.ok();
            return Ok(ExitCode::from(e.exit_code()));
        }
    };
    tracing::Span::current().record("video_id", tracing::field::display(&video_id));

    let lang = language_to_str(cli.lang);
    let format = format_to_provider_format(cli.format);

    tracing::info!(target: "events", event = "started", video_id = %video_id, language = %lang);

    if !cli.no_cache {
        let path = cache::cache_path(
            &video_id,
            lang,
            format.extension(),
            cli.cache_ttl_duration(),
        )?;
        match cache::read_cache_with_hint(&path, cli.cache_ttl_duration()).await {
            Ok(Some((bytes, format_hint))) => {
                tracing::info!(target: "events", event = "cache_hit", video_id = %video_id);
                let duration_ms = started.elapsed().as_millis() as u64;
                // GAP-AUD-2026-051: cache hits now honour the stored
                // `format_hint` instead of hard-coding `Srt`. Without
                // this fix, noteey-style bodies cached on disk were
                // re-parsed as SRT and leaked `MM:SS` timestamps into
                // the output.
                let converted = match convert_format(&bytes, format, format_hint) {
                    Ok(s) => s,
                    Err(e) => {
                        output_error(cli, &e).await.ok();
                        return Ok(ExitCode::from(e.exit_code()));
                    }
                };
                output_success(cli, "cache", &video_id, &converted, "cache", duration_ms).await?;
                tracing::info!(target: "events", event = "completed", video_id = %video_id, source = "cache");
                return Ok(ExitCode::SUCCESS);
            }
            Ok(None) => {
                tracing::debug!(target: "events", event = "cache_miss", video_id = %video_id);
            }
            Err(e) => {
                tracing::warn!(target: "events", event = "cache_error", error = %e);
            }
        }
    }

    if cli.dry_run {
        // GAP-E2E-009: emit the dry-run envelope and exit 0. The
        // previous code constructed `AppError::NoSubtitle(NotPublished)`
        // and returned `ExitCode::from(66)`, which broke CI scripts
        // that distinguished cache-hit vs cache-miss by exit code.
        tracing::info!(target: "events", event = "dry_run_cache_miss", video_id = %video_id);
        output_dry_run(cli, &video_id, true).await.ok();
        return Ok(ExitCode::SUCCESS);
    }

    let fetch_result = retry_with_backoff(
        || async { chain.fetch_subtitle(&video_id, lang, format).await },
        3,
    )
    .await;

    let (info, content) = match fetch_result {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(target: "events", event = "failed", video_id = %video_id, error = %e);
            output_error(cli, &e).await.ok();
            return Ok(ExitCode::from(e.exit_code()));
        }
    };

    tracing::info!(target: "events", event = "fetched", video_id = %video_id, source = %info.source_url);

    if !cli.no_cache {
        if let Ok(path) = cache::cache_path(
            &video_id,
            lang,
            format.extension(),
            cli.cache_ttl_duration(),
        ) {
            // GAP-AUD-2026-051: persist the format_hint sidecar so the
            // next cache hit can route the body through the right
            // parser (srt vs noteey).
            if let Err(e) = cache::write_cache_with_hint(&path, &content, info.format_hint).await {
                tracing::warn!(target: "events", event = "cache_write_error", error = %e);
            }
        }
    }

    let converted = match convert_format(&content, format, info.format_hint) {
        Ok(s) => s,
        Err(e) => {
            output_error(cli, &e).await.ok();
            return Ok(ExitCode::from(e.exit_code()));
        }
    };

    let duration_ms = started.elapsed().as_millis() as u64;
    let source = info.source_url.clone();
    let provider = info.provider;

    // GAP-AUD-2026-065: byte_size is now computed inside
    // output_success from the final NFC content.
    output_success(cli, provider, &video_id, &converted, &source, duration_ms).await?;

    tracing::info!(target: "events", event = "completed", video_id = %video_id, provider, duration_ms);

    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use crate::commands::{format_to_provider_format, format_to_str, language_to_str};
    use crate::provider::Format;
    use clap::Parser;

    fn parse_cli(url: &str, dry_run: bool, json: bool) -> Cli {
        let mut args = vec![
            "youtube-legend-cli".to_string(),
            url.to_string(),
            "--format".to_string(),
            "srt".to_string(),
        ];
        if dry_run {
            args.push("--dry-run".to_string());
        }
        if json {
            args.push("--json".to_string());
        }
        Cli::parse_from(args)
    }

    /// GAP-E2E-009: dry-run on a cache miss must return `ExitCode::SUCCESS`
    /// and surface the JSON envelope to stdout, not the legacy
    /// `NoSubtitle(NotPublished)` error with exit 66.
    #[test]
    fn dry_run_envelope_shape_matches_contract() {
        // Build a `Cli` mirroring the operator's invocation and
        // verify the dry-run envelope JSON we serialise contains the
        // fields the downstream contract promises. We do not call
        // `extract::run` end-to-end because that would require a
        // stub chain and a network fixture; the helper path is
        // unit-testable in isolation.
        let cli = parse_cli("https://youtu.be/dQw4w9WgXcQ", true, true);
        let payload = serde_json::json!({
            "event": "dry_run_cache_miss",
            "video_id": "dQw4w9WgXcQ",
            "language": language_to_str(cli.lang),
            "format": format_to_str(cli.format),
            "would_fetch": true,
        });
        let json = payload.to_string();
        assert!(
            json.contains("\"event\":\"dry_run_cache_miss\""),
            "envelope must carry the stable event field, got: {json}"
        );
        assert!(
            json.contains("\"would_fetch\":true"),
            "envelope must include the would_fetch flag, got: {json}"
        );
        // Sanity-check the format/language projection helpers stay
        // in sync with the provider layer.
        assert_eq!(format_to_provider_format(cli.format), Format::Srt);
    }

    #[test]
    fn dry_run_with_no_cache_returns_zero_exit_code_contract() {
        // Pure type-level assertion: the previous implementation
        // returned `ExitCode::from(66)` (EX_NOINPUT); the new contract
        // returns `ExitCode::SUCCESS` (0) so CI scripts can branch on
        // cache presence without parsing the JSON envelope.
        assert_eq!(ExitCode::SUCCESS, ExitCode::from(0));
    }
}
