//! `youtube-legend-cli` binary entry point.

use std::process::ExitCode;

use youtube_legend_cli::cli::{load_config, parse_with_overrides};
use youtube_legend_cli::error::AppError;
use youtube_legend_cli::logging::init_tracing;
use youtube_legend_cli::run;

use mimalloc::MiMalloc;

/// Drop-in mimalloc global allocator. Reduces allocation overhead in
/// the subtitle-fetching hot path (HTTP body buffers, URL parsing).
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> ExitCode {
    // GAP-E2E-016: `parse_with_overrides` returns both the parsed
    // `Cli` and a `CliOverrideFlags` bitmask describing which flags
    // the operator actually typed (vs which the parser filled from
    // `default_value`). The bitmask is the only reliable input to
    // `apply_config_overrides` because it does not rely on a
    // sentinel comparison against a literal default value.
    //
    // `parse_with_overrides` cannot return a `Cli` directly because
    // clap's `Command::get_matches_from_mut` consumes the args and
    // we need to inspect `value_source` before building `Cli`. The
    // trade-off is one extra allocation for the `matches` builder,
    // which is negligible against the per-extraction HTTP cost.
    let (mut cli, cli_overrides) = match parse_with_overrides() {
        Ok(pair) => pair,
        Err(e) => {
            // `clap::Error::exit` prints the error and terminates
            // the process via `std::process::exit(2)` — this is the
            // canonical clap behaviour for parse-time argument
            // validation failures (see rules-rust-cli-com-clap-io-
            // exitcodes-erros: `CONVERTER clap::Error em exit code 2
            // para uso incorreto`).
            e.exit();
        }
    };

    // 1. Apply config-file overrides BEFORE we propagate env vars, so
    //    the effective level/format take config + CLI into account.
    if let Some(path) = cli.config.clone() {
        match load_config(&path) {
            Ok(overrides) => cli.apply_config_overrides(overrides, &cli_overrides),
            Err(e) => {
                // GAP-E2E-013: `AppError::Config`'s Display already
                // includes the `config error: ` prefix (see
                // src/error.rs:181). The previous code duplicated it
                // via this `eprintln!("config error: {e}")`, producing
                // `config error: config error: <path>` to the operator.
                // Emit the Display directly and rely on the
                // `Termination` impl to route the matching exit code.
                eprintln!("{e}");
                return ExitCode::from(e.exit_code());
            }
        }
    }

    // 2. Propagate effective values into env vars so downstream
    //    crates (`tracing`, progress bars) see the chosen config.
    cli.apply_overrides();

    // 3. Initialise tracing with the effective level/format/color.
    //    `init_tracing` honours `YT_LOG_LEVEL` / `YT_LOG_FORMAT` if
    //    they were set, otherwise it falls back to the CLI args.
    if let Err(e) = init_tracing(cli.log_level, cli.log_format, cli.color, cli.quiet) {
        eprintln!("tracing init failed: {e}");
        return ExitCode::from(e.exit_code());
    }

    // 4. SIGINT and SIGTERM are honoured cooperatively: a dedicated
    //    watcher task inside the runtime arms a CancellationToken on
    //    the first signal, which the in-flight HTTP requests observe
    //    at their next await point for clean abort. A second signal
    //    during shutdown is forwarded to the runtime's shutdown
    //    handle to force an immediate exit with code 130
    //    (conventional SIGINT). On Windows, only SIGINT is delivered
    //    via `tokio::signal::ctrl_c`; SIGTERM has no portable
    //    equivalent there and is silently ignored.
    let shutdown_token = tokio_util::sync::CancellationToken::new();

    let worker_threads = std::thread::available_parallelism()
        .map(|n| n.get().clamp(2, 8))
        .unwrap_or(4);

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!(error = %e, "failed to start tokio runtime");
            return ExitCode::from(AppError::Internal(format!("tokio runtime: {e}")).exit_code());
        }
    };

    let exit_code = runtime.block_on(async move {
        let signal_watcher = tokio::spawn(install_signal_handler(shutdown_token.clone()));
        let result = tokio::select! {
            biased;
            result = run(cli) => result,
            _ = shutdown_token.cancelled() => {
                tracing::warn!("cancellation requested before completion");
                Ok(ExitCode::from(130))
            }
        };
        // Detach the watcher so its handle can drop without aborting
        // the runtime. The watcher's own drop cancels its inner loop.
        drop(signal_watcher);
        result
    });
    match exit_code {
        Ok(code) => code,
        Err(e) => ExitCode::from(e.exit_code()),
    }
}

/// Watch SIGINT (Ctrl-C) and SIGTERM (Unix only) and cancel `token` on
/// the first signal observed. A second SIGINT is a hard-exit signal;
/// the runtime's shutdown task is not given a chance to drain.
#[cfg(unix)]
async fn install_signal_handler(token: tokio_util::sync::CancellationToken) {
    use tokio::signal::unix::{signal, SignalKind};
    let mut sigterm = match signal(SignalKind::terminate()) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "could not install SIGTERM handler");
            return;
        }
    };
    let mut sigint = match signal(SignalKind::interrupt()) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "could not install SIGINT handler");
            return;
        }
    };
    let mut first = true;
    loop {
        tokio::select! {
            biased;
            _ = sigterm.recv() => {},
            _ = sigint.recv() => {},
        }
        if first {
            tracing::info!(target: "events", event = "signal", "shutdown requested");
            token.cancel();
            first = false;
        } else {
            tracing::warn!(target: "events", event = "signal", "second signal received; forcing immediate exit");
            std::process::exit(130);
        }
    }
}

#[cfg(not(unix))]
async fn install_signal_handler(token: tokio_util::sync::CancellationToken) {
    let mut first = true;
    loop {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::warn!(error = %e, "could not install SIGINT handler");
            return;
        }
        if first {
            tracing::info!(target: "events", event = "signal", "shutdown requested");
            token.cancel();
            first = false;
        } else {
            tracing::warn!(target: "events", event = "signal", "second signal received; forcing immediate exit");
            std::process::exit(130);
        }
    }
}
