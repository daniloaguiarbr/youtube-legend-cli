//! Batch extraction command: read N URLs from stdin, process each, emit
//! a concatenated or JSON-per-line result.

use crate::cache;
use crate::cli::Cli;
use crate::commands::{
    convert_format, format_to_provider_format, language_to_str, output_error, output_success,
    parse_video_id_from_url,
};
use crate::error::{AppError, AppResult};
use crate::io;
use crate::provider::ProviderChain;
use crate::retry::retry_with_backoff;
use std::collections::HashSet;
use std::process::ExitCode;
use std::time::Instant;
use tracing::instrument;

/// Run the batch extraction flow.
///
/// Reads URLs from stdin, deduplicates them, and processes each in
/// order. Continues on per-item errors, emitting the structured JSON
/// envelope for each failure. Returns the exit code of the first
/// failure when *every* item failed; otherwise returns 0.
///
/// # Cancel safety
///
/// Cancel-safe. Each per-item loop iteration is wrapped in `tokio::select!`
/// semantics through the underlying provider call: a drop before
/// completion aborts at the next `await` and leaves stdout untouched for
/// that item.
///
/// # Errors
///
/// - [`AppError::InvalidUsage`] / [`AppError::StdinEmpty`] when stdin is
///   a TTY or empty.
#[instrument(skip(cli, chain), fields(total))]
pub async fn run(cli: &Cli, chain: &ProviderChain) -> AppResult<ExitCode> {
    let urls = io::read_urls_from_stdin().await?;
    tracing::Span::current().record("total", urls.len());

    let mut seen: HashSet<String> = HashSet::new();
    let unique: Vec<String> = urls
        .into_iter()
        .filter(|u| seen.insert(u.clone()))
        .collect();

    let total = unique.len();
    let mut last_err: Option<AppError> = None;
    let mut succeeded = 0u32;
    let lang = language_to_str(cli.lang);
    let format = format_to_provider_format(cli.format);
    let cache_ttl = cli.cache_ttl_duration();

    tracing::info!(target: "events", event = "batch_started", total = total);

    for (idx, url) in unique.iter().enumerate() {
        if !cli.quiet {
            tracing::info!(target: "events", event = "progress", index = idx + 1, total, url = %url);
        }

        let video_id = match parse_video_id_from_url(cli, url) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!(target: "events", event = "failed", url = %url, error = %e);
                output_error(cli, &e).await.ok();
                last_err = Some(e);
                continue;
            }
        };

        let started = Instant::now();

        if !cli.no_cache {
            if let Ok(path) = cache::cache_path(&video_id, lang, format.extension(), cache_ttl) {
                match cache::read_cache(&path, cache_ttl).await {
                    Ok(Some(bytes)) => {
                        let bytes_len = bytes.len() as u64;
                        let duration_ms = started.elapsed().as_millis() as u64;
                        match convert_format(&bytes, format) {
                            Ok(converted) => {
                                write_item(
                                    cli,
                                    idx,
                                    total,
                                    &video_id,
                                    &converted,
                                    "cache",
                                    bytes_len,
                                    duration_ms,
                                )
                                .await?;
                                succeeded += 1;
                                continue;
                            }
                            Err(e) => {
                                output_error(cli, &e).await.ok();
                                last_err = Some(e);
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(_) => {}
                }
            }
        }

        if cli.dry_run {
            let err = AppError::NoSubtitle(crate::error::NoSubtitleReason::NotPublished);
            tracing::info!(target: "events", event = "dry_run_cache_miss", video_id = %video_id);
            output_error(cli, &err).await.ok();
            last_err = Some(err);
            continue;
        }

        let result = retry_with_backoff(
            || async { chain.fetch_subtitle(&video_id, lang, format).await },
            3,
        )
        .await;

        match result {
            Ok((info, content)) => {
                tracing::info!(target: "events", event = "fetched", video_id = %video_id, source = %info.source_url);

                if !cli.no_cache {
                    if let Ok(path) =
                        cache::cache_path(&video_id, lang, format.extension(), cache_ttl)
                    {
                        let _ = cache::write_cache(&path, &content).await;
                    }
                }

                let converted = match convert_format(&content, format) {
                    Ok(s) => s,
                    Err(e) => {
                        output_error(cli, &e).await.ok();
                        last_err = Some(e);
                        continue;
                    }
                };
                let bytes = content.len() as u64;
                let duration_ms = started.elapsed().as_millis() as u64;
                let source = info.source_url.clone();

                match write_item(
                    cli,
                    idx,
                    total,
                    &video_id,
                    &converted,
                    &source,
                    bytes,
                    duration_ms,
                )
                .await
                {
                    Ok(()) => succeeded += 1,
                    Err(e) => {
                        output_error(cli, &e).await.ok();
                        last_err = Some(e);
                    }
                }
            }
            Err(e) => {
                tracing::error!(target: "events", event = "failed", video_id = %video_id, error = %e);
                output_error(cli, &e).await.ok();
                last_err = Some(e);
            }
        }
    }

    if !cli.quiet {
        tracing::info!(target: "events", event = "batch_completed", succeeded, total);
    }

    match last_err {
        Some(e) if succeeded == 0 => Ok(ExitCode::from(e.exit_code())),
        _ => Ok(ExitCode::SUCCESS),
    }
}

#[allow(clippy::too_many_arguments)]
async fn write_item(
    cli: &Cli,
    idx: usize,
    total: usize,
    video_id: &str,
    converted: &str,
    source: &str,
    bytes: u64,
    duration_ms: u64,
) -> AppResult<()> {
    if cli.json {
        output_success(cli, video_id, converted, source, bytes, duration_ms).await?;
    } else {
        let separator = if idx > 0 { "\n---\n" } else { "" };
        let mut buf = separator.as_bytes().to_vec();
        buf.extend_from_slice(converted.as_bytes());
        buf.push(b'\n');
        io::write_subtitle_to_stdout(&buf).await?;
    }
    let _ = total;
    Ok(())
}
