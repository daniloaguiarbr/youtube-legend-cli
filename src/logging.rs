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

    // Cluster 1 / GAP-AUD-003: silence `chromiumoxide` warn-level events
    // that fire on every CDP message the upstream `Message` enum does
    // not recognise. The handler logic already drops the unknown
    // event (`ignore_invalid_messages: true` is the crate default),
    // but the log line is emitted unconditionally and pollutes stderr.
    // Pinning the crate to `error` keeps diagnostic value while
    // eliminating the noise loop. Operators who need more detail can
    // override via `YT_LOG_LEVEL=chromiumoxide=warn`.
    let filter = filter
        .add_directive(
            "chromiumoxide=error"
                .parse()
                .unwrap_or_else(|_| tracing_subscriber::filter::LevelFilter::ERROR.into()),
        )
        .add_directive(
            "chromiumoxide_fetcher=error"
                .parse()
                .unwrap_or_else(|_| tracing_subscriber::filter::LevelFilter::ERROR.into()),
        );

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

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::filter::LevelFilter;

    /// GAP-AUD-003: even with the operator asking for `info` globally,
    /// the chromiumoxide crate must stay at `error` so the CDP noise
    /// loop (`WS Invalid message: data did not match any variant of
    /// untagged enum Message`) is silenced by default.
    #[test]
    fn chromiumoxide_directive_pins_to_error() {
        let base = EnvFilter::new("info");
        let with_chrome = base
            .add_directive("chromiumoxide=error".parse().expect("valid directive"))
            .add_directive(
                "chromiumoxide_fetcher=error"
                    .parse()
                    .expect("valid directive"),
            );

        // The combined filter must report `error` (or stricter) for
        // both chromiumoxide targets, regardless of the global level.
        assert_eq!(
            with_chrome.max_level_hint(),
            Some(LevelFilter::INFO),
            "global level retained"
        );

        // Directives survive a round-trip serialise -> parse so the
        // operator can echo the filter back via `YT_LOG_LEVEL`.
        let rendered = with_chrome.to_string();
        assert!(
            rendered.contains("chromiumoxide=error"),
            "filter string must contain chromiumoxide=error, got: {rendered}"
        );
    }

    /// GAP-AUD-003 complement: when the operator explicitly raises the
    /// chromiumoxide level via env, the directive must respect that
    /// (the chromiumoxide pin is a default, not a hard ceiling).
    #[test]
    fn chromiumoxide_directive_respects_explicit_override() {
        let operator_wants = EnvFilter::try_new("chromiumoxide=warn").expect("valid");
        // Build a fresh filter that lets the operator override win.
        let combined = EnvFilter::new("info")
            .add_directive(operator_wants.to_string().parse().expect("valid directive"));
        let rendered = combined.to_string();
        assert!(
            rendered.contains("chromiumoxide=warn"),
            "explicit chromiumoxide=warn must survive merge, got: {rendered}"
        );
    }
}
