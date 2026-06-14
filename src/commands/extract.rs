//! Single-URL extraction command: read one URL, fetch, convert, write.

use crate::cache;
use crate::cli::Cli;
use crate::commands::{
    convert_format, extract_url_from_input, format_to_provider_format, language_to_str,
    output_error, output_success, parse_video_id_from_url,
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

    let url = extract_url_from_input(cli).await?;
    let video_id = parse_video_id_from_url(cli, &url)?;
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
        match cache::read_cache(&path, cli.cache_ttl_duration()).await {
            Ok(Some(bytes)) => {
                tracing::info!(target: "events", event = "cache_hit", video_id = %video_id);
                let duration_ms = started.elapsed().as_millis() as u64;
                let bytes_len = bytes.len() as u64;
                let converted = convert_format(&bytes, format)?;
                output_success(cli, &video_id, &converted, "cache", bytes_len, duration_ms).await?;
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
        let err = crate::error::AppError::NoSubtitle(crate::error::NoSubtitleReason::NotPublished);
        tracing::info!(target: "events", event = "dry_run_cache_miss", video_id = %video_id);
        output_error(cli, &err).await.ok();
        return Ok(ExitCode::from(err.exit_code()));
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
            if let Err(e) = cache::write_cache(&path, &content).await {
                tracing::warn!(target: "events", event = "cache_write_error", error = %e);
            }
        }
    }

    let converted = match convert_format(&content, format) {
        Ok(s) => s,
        Err(e) => {
            output_error(cli, &e).await.ok();
            return Ok(ExitCode::from(e.exit_code()));
        }
    };

    let bytes = content.len() as u64;
    let duration_ms = started.elapsed().as_millis() as u64;
    let source = info.source_url.clone();

    output_success(cli, &video_id, &converted, &source, bytes, duration_ms).await?;

    tracing::info!(target: "events", event = "completed", video_id = %video_id, duration_ms, bytes);

    Ok(ExitCode::SUCCESS)
}
