//! Example: emit a single JSON envelope to stdout.
//!
//! Run with:
//!
//! ```text
//! cargo run --example json_output -- "https://youtu.be/dQw4w9WgXcQ"
//! ```
//!
//! The example passes `--json` to [`Cli::parse_from`]. The subtitle
//! body is then written as a single JSON object with `video_id`,
//! `language`, `format`, `content`, `bytes`, `duration_ms`, and
//! `source` keys.

use clap::Parser;
use std::process::ExitCode;
use youtube_legend_cli::{run, Cli};

#[tokio::main]
async fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let cli = Cli::parse_from(
        std::iter::once("json_output".to_string())
            .chain(argv.into_iter().skip(1).chain(["--json".to_string()])),
    );
    match run(cli).await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(e.exit_code())
        }
    }
}
