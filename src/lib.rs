//! `youtube-legend-cli` — a non-interactive Rust CLI for downloading
//! YouTube subtitles through third-party providers, using a native Unix
//! `stdin`/`stdout` interface.
//!
//! # Quickstart
//!
//! The fastest way to drive the crate is to construct a [`Cli`] with
//! `Cli::parse()` (as `main.rs` does) and forward it to [`run`]:
//!
//! ```no_run
//! use clap::Parser;
//! use youtube_legend_cli::{run, Cli};
//!
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let cli = Cli::parse_from(["demo", "https://youtu.be/dQw4w9WgXcQ"]);
//! let exit_code = run(cli).await?;
//! # let _ = exit_code;
//! # Ok(()) }
//! ```
//!
//! # Architecture
//!
//! - [`cli`] — `clap`-derived argument parser and the [`Cli`] struct.
//! - [`commands`] — top-level dispatch between single-URL extract and
//!   batch mode.
//! - `provider` — the `Provider` trait and the two concrete
//!   implementations, plus the `ProviderChain` that walks
//!   them with one-request-per-second throttling.
//! - [`parse`] — SRT text extraction and YouTube URL/video-id parsing.
//! - [`cache`] — TTL-based local file cache keyed on
//!   `(video_id, language, format)`.
//! - [`retry`] — exponential backoff helper and in-memory circuit
//!   breaker.
//! - [`io`] — `stdin`/`stdout`/TTY helpers.
//! - [`error`] — the [`error::AppError`] enum and the process exit-code
//!   table.
//! - [`logging`] — initialiser for the global `tracing` subscriber.
//! - `text` — Unicode NFC normalisation.
//! - [`crypto`] — AES-256-CBC + PBKDF2 used by provider-B's request
//!   signing path.
//!
//! # Stream contracts
//!
//! - `stdout` is reserved exclusively for the subtitle body (or the
//!   `--json` envelope).
//! - `stderr` is reserved exclusively for logs, progress, and human
//!   error messages.
//! - `stdin` accepts a single URL, a batch of one-URL-per-line, or
//!   `--batch` flag input.
//!
//! # Cancellation
//!
//! `SIGINT` and `SIGTERM` are wired through `tokio_util::CancellationToken`
//! in `main.rs`. In-flight requests are allowed to complete; the process
//! exits with code 130. The async API exposed by this crate is
//! cancellation-safe at every public await point.
//!
//! # Minimum supported Rust version (MSRV)
//!
//! `1.88.0` — declared in `Cargo.toml` `rust-version` field.
//! The local toolchain pinned via `rust-toolchain.toml` may be
//! newer; the MSRV in `Cargo.toml` is the contract with users.

#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
#![warn(rustdoc::private_intra_doc_links)]
#![warn(rustdoc::redundant_explicit_links)]
#![warn(rustdoc::unescaped_backticks)]
#![warn(rustdoc::invalid_codeblock_attributes)]
#![deny(rustdoc::invalid_html_tags)]
#![deny(rustdoc::invalid_rust_codeblocks)]
#![deny(rustdoc::bare_urls)]
#![deny(rustdoc::private_doc_tests)]
#![warn(clippy::undocumented_unsafe_blocks)]

/// Cache subsystem: TTL-keyed local files for fetched subtitles.
pub mod cache;

/// Command-line argument parser (clap derive).
pub mod cli;

/// Top-level command dispatch: extract one URL, or batch many.
pub mod commands;

/// AES-256-CBC + PBKDF2 token encryption used by provider-B's request
/// signing path. Do not use these primitives for new code outside the
/// provider-B compatibility layer.
pub mod crypto;

/// Error types and process exit-code table.
pub mod error;

/// Stdin / stdout / TTY helpers.
pub mod io;

/// `tracing` subscriber initialiser.
pub mod logging;

/// SRT text extraction and YouTube URL / video-id parsing.
pub mod parse;

/// Provider trait, two concrete implementations, and the provider chain
/// with throttling.
pub mod provider;

/// Exponential-backoff retry helper and in-memory circuit breaker.
pub mod retry;

/// Internal constants for provider hosts, paths, cookies, and user
/// agent. Gitignored; never published. Consumed by `provider_a`,
/// `provider_b`, and the `snapshot` binary.
pub(crate) mod secret_endpoints;

/// Unicode NFC normalisation.
pub(crate) mod text;

use std::process::ExitCode;

pub use cli::{Cli, FormatArg, LanguageArg};
pub use error::{AppError, AppResult, NoSubtitleReason};

/// Top-level entry point that wires the parsed [`Cli`] to the command
/// dispatch. See `main.rs` for the binary that calls this.
///
/// # Errors
///
/// - [`error::AppError::InvalidUsage`] when the CLI was validated and
///   found to have a bad combination of flags.
/// - Any of the provider / network / cache / IO errors described in
///   [`error::AppError`].
///
/// # Cancel safety
///
/// This future is cancel-safe: dropping it before completion does not
/// leak cache writes or partial fetches; the in-flight HTTP request is
/// aborted at the next `await` point.
pub async fn run(cli: Cli) -> Result<ExitCode, error::AppError> {
    commands::run(cli).await
}
