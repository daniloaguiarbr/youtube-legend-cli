//! Example: download subtitles for a single YouTube URL.
//!
//! Run with:
//!
//! ```text
//! cargo run --example single_url -- "https://youtu.be/dQw4w9WgXcQ"
//! ```
//!
//! The program forwards a synthetic argv to `Cli::parse_from` so the
//! full flag surface of the production CLI is available to the
//! example. The body is printed to stdout by [`youtube_legend_cli::run`].

use clap::Parser;
use std::process::ExitCode;
use youtube_legend_cli::{run, Cli};

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse_from(std::env::args().take_while(|a| a != "--"));
    match run(cli).await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(e.exit_code())
        }
    }
}
