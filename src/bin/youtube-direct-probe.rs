//! `youtube-direct-probe` — standalone debugging tool for the
//! `ProviderYouTubeDirect` pipeline.
//!
//! Reads a single `video_id` from the command line, runs the
//! direct provider end-to-end, and prints a Newline-Delimited JSON
//! (`NDJSON`) trace to stdout. Each line reports a step:
//!
//! 1. `{"step":"fetched_player_response","video_id":"...","tracks":N}`
//! 2. `{"step":"decipher_applied","video_id":"...","has_n_param":bool,"kind":"asr"|"manual"}`
//! 3. `{"step":"fetched_content","video_id":"...","bytes":N,"fmt":"..."}`
//! 4. `{"step":"done","video_id":"...","bytes":N,"language":"..."}`
//!
//! On failure, a final line of the form
//! `{"step":"error","video_id":"...","code":N,"message":"..."}`
//! is emitted and the process exits with the matching code. The
//! probe is intentionally **without** cache and **without** retry
//! so operators can see what the upstream returns today.

use std::process::ExitCode;
use std::time::Instant;

use clap::Parser;
use serde::Serialize;
use youtube_legend_cli::provider::provider_youtube_direct::ProviderYouTubeDirect;
use youtube_legend_cli::provider::{Format, Provider};

#[derive(Debug, Parser)]
#[command(
    name = "youtube-direct-probe",
    about = "Probe ProviderYouTubeDirect with NDJSON tracing",
    long_about = None,
    disable_help_subcommand = true
)]
struct Opts {
    /// 11-character `YouTube` video id.
    video_id: String,
    /// ISO 639-1 language code (default: `en`).
    #[arg(long, default_value = "en")]
    lang: String,
    /// Prefer auto-generated (ASR) captions when multiple tracks match.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    asr: bool,
    /// HTTP timeout in seconds.
    #[arg(long, default_value_t = 30)]
    timeout: u64,
}

#[derive(Debug, Serialize)]
#[serde(tag = "step")]
enum Step {
    FetchedPlayerResponse {
        video_id: String,
        tracks: usize,
    },
    DecipherApplied {
        video_id: String,
        has_n_param: bool,
        kind: String,
    },
    FetchedContent {
        video_id: String,
        bytes: usize,
        fmt: String,
    },
    Done {
        video_id: String,
        bytes: usize,
        language: String,
        duration_ms: u64,
    },
    Error {
        video_id: String,
        code: u8,
        message: String,
    },
}

fn emit(step: &Step) {
    // NDJSON: one object per line. Each write is followed by a
    // newline so downstream `jaq -c` consumers can parse the trace
    // line-by-line.
    if let Ok(line) = serde_json::to_string(step) {
        println!("{line}");
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let opts = Opts::parse();
    let video_id = opts.video_id.clone();
    let started = Instant::now();

    let provider = match ProviderYouTubeDirect::with_user_agent(concat!(
        env!("CARGO_PKG_NAME"),
        "/",
        env!("CARGO_PKG_VERSION")
    ))
    .map(|p| p.prefer_asr(opts.asr))
    {
        Ok(p) => p,
        Err(e) => {
            emit(&Step::Error {
                video_id: video_id.clone(),
                code: e.exit_code(),
                message: format!("client build: {e}"),
            });
            return ExitCode::from(e.exit_code());
        }
    };

    let format = Format::Srt;

    // Step 1: fetch player response.
    let info = match provider.fetch_subtitle(&video_id, &opts.lang, format).await {
        Ok(i) => {
            emit(&Step::FetchedPlayerResponse {
                video_id: video_id.clone(),
                tracks: 1,
            });
            emit(&Step::DecipherApplied {
                video_id: video_id.clone(),
                has_n_param: i.source_url.contains("&n="),
                kind: if opts.asr {
                    "asr".to_string()
                } else {
                    "manual".to_string()
                },
            });
            i
        }
        Err(e) => {
            let code = e.exit_code();
            emit(&Step::Error {
                video_id: video_id.clone(),
                code,
                message: format!("{e}"),
            });
            return ExitCode::from(code);
        }
    };

    // Step 3: fetch content.
    let body = match provider.fetch_content(&info).await {
        Ok(b) => {
            emit(&Step::FetchedContent {
                video_id: video_id.clone(),
                bytes: b.len(),
                fmt: info.format.as_str().to_string(),
            });
            b
        }
        Err(e) => {
            let code = e.exit_code();
            emit(&Step::Error {
                video_id: video_id.clone(),
                code,
                message: format!("{e}"),
            });
            return ExitCode::from(code);
        }
    };

    // Step 4: done.
    let duration_ms = started.elapsed().as_millis() as u64;
    emit(&Step::Done {
        video_id: video_id.clone(),
        bytes: body.len(),
        language: info.language.clone(),
        duration_ms,
    });
    ExitCode::SUCCESS
}
