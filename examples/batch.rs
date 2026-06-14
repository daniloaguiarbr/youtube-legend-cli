//! Example: read URLs from stdin in batch mode.
//!
//! Run with:
//!
//! ```text
//! cat urls.txt | cargo run --example batch
//! ```
//!
//! `urls.txt` is one URL per line. The example passes `--batch` and
//! `--json` to [`Cli::parse_from`], then forwards the parsed
//! configuration to [`youtube_legend_cli::run`]. Each subtitle is
//! emitted as a separate JSON object on stdout.

use clap::Parser;
use std::process::ExitCode;
use youtube_legend_cli::{run, Cli};

#[tokio::main]
async fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let cli = Cli::parse_from(
        std::iter::once("batch".to_string()).chain(
            argv.into_iter()
                .skip(1)
                .chain(["--batch".to_string(), "--json".to_string()]),
        ),
    );
    match run(cli).await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(e.exit_code())
        }
    }
}
