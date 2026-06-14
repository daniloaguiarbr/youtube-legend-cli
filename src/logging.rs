//! `tracing` subscriber initialiser.
//!
//! Honours the propagated env vars set by `Cli::apply_overrides`:
//! - `YT_LOG_LEVEL` overrides `RUST_LOG` and the CLI default.
//! - `YT_LOG_FORMAT` selects text or json output.
//! - `NO_COLOR` and `CLICOLOR_FORCE` (set by `--color`) gate ANSI.

use crate::cli::{ColorArg, LogFormatArg, LogLevelArg};
use crate::error::{AppError, AppResult};
use std::io::IsTerminal;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initialise the global `tracing` subscriber.
///
/// Precedence (highest first):
/// 1. `YT_LOG_LEVEL` env var (set by `Cli::apply_overrides` when the
///    user passes `--log-level`).
/// 2. `RUST_LOG` env var.
/// 3. CLI `log_level` argument (passed via `cli.log_level`).
/// 4. Default `error`.
///
/// # Errors
///
/// - [`AppError::Internal`] when the global subscriber cannot be
///   installed (typically because another test already installed one).
pub fn init_tracing(
    cli_log_level: LogLevelArg,
    cli_log_format: LogFormatArg,
    cli_color: ColorArg,
    quiet: bool,
) -> AppResult<()> {
    let filter = if let Ok(env_level) = std::env::var("YT_LOG_LEVEL") {
        EnvFilter::try_new(env_level).unwrap_or_else(|_| EnvFilter::new("error"))
    } else if let Ok(rust_log) = std::env::var("RUST_LOG") {
        EnvFilter::try_new(rust_log).unwrap_or_else(|_| EnvFilter::new("error"))
    } else if quiet {
        EnvFilter::new("error")
    } else {
        EnvFilter::new(cli_log_level.as_str())
    };

    let registry = tracing_subscriber::registry().with(filter);

    let use_json = matches!(std::env::var("YT_LOG_FORMAT").ok().as_deref(), Some("json"))
        || matches!(cli_log_format, LogFormatArg::Json);

    if use_json {
        let layer = fmt::layer()
            .json()
            .with_writer(std::io::stderr)
            .with_target(false)
            .with_current_span(false)
            .with_ansi(false);
        registry
            .with(layer)
            .try_init()
            .map_err(|e| AppError::Internal(format!("tracing init failed: {e}")))?;
    } else {
        let ansi = match cli_color {
            ColorArg::Never => false,
            ColorArg::Always => true,
            ColorArg::Auto => std::io::stderr().is_terminal(),
        };
        let layer = fmt::layer()
            .with_writer(std::io::stderr)
            .with_target(false)
            .with_ansi(ansi);
        registry
            .with(layer)
            .try_init()
            .map_err(|e| AppError::Internal(format!("tracing init failed: {e}")))?;
    }

    Ok(())
}
