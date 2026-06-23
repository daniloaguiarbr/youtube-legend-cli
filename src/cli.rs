//! Command-line argument parsing via [`clap`] derive.
//!
//! The single entry point is [`Cli`]. Construct one with `Cli::parse()` at
//! the start of `main`, validate it with [`Cli::validate`], then dispatch
//! the rest of the program.

use crate::error::{AppError, AppResult};
use clap::{ArgAction, Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Output format for the subtitle body.
///
/// `Txt` strips SRT timestamps and joins cues with blank lines. `Srt`
/// returns the raw subtitle text exactly as the provider delivered it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[non_exhaustive]
pub enum FormatArg {
    /// Plain text with timestamps removed.
    Txt,
    /// Raw `SubRip` text with timestamps preserved.
    Srt,
}

/// Preferred subtitle language. Maps to ISO 639-1 primary subtags.
///
/// The CLI accepts full IETF BCP 47 locales (FR-009): `pt-BR`,
/// `pt_BR.UTF-8`, and `EN-us` all normalise to their primary subtag
/// before matching one of these variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[non_exhaustive]
pub enum LanguageArg {
    /// English (`en`).
    En,
    /// Brazilian / European Portuguese (`pt`).
    Pt,
    /// Spanish (`es`).
    Es,
    /// French (`fr`).
    Fr,
    /// German (`de`).
    De,
    /// Italian (`it`).
    It,
}

/// Tracing log level. Maps to canonical `tracing` level names so
/// downstream subscribers parse the value without translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[non_exhaustive]
pub enum LogLevelArg {
    /// Only errors are reported.
    Error,
    /// Warnings and errors.
    Warn,
    /// Informational messages and above (default).
    Info,
    /// Diagnostic detail for debugging.
    Debug,
    /// Maximum verbosity for trace-level inspection.
    Trace,
}

impl LogLevelArg {
    /// Canonical `tracing` level name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

impl LogFormatArg {
    /// Lowercase formatter name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Json => "json",
        }
    }
}

impl ColorArg {
    /// Lowercase colour policy name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Always => "always",
            Self::Never => "never",
        }
    }
}

/// Format for log output. `text` is human-readable; `json` produces
/// structured log records suitable for ingestion by aggregators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[non_exhaustive]
pub enum LogFormatArg {
    /// Human-readable, line-oriented text (default).
    Text,
    /// Structured JSON log records, one per line.
    Json,
}

/// Color policy for terminal output. Mirrors the `clap` convention
/// used by `ripgrep`, `bat`, and other modern CLIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[non_exhaustive]
pub enum ColorArg {
    /// Honour `NO_COLOR`, `CLICOLOR_FORCE`, and TTY detection (default).
    Auto,
    /// Always emit ANSI colour escapes.
    Always,
    /// Never emit ANSI colour escapes.
    Never,
}

/// Provider selection strategy for the subtitle-fetch chain.
///
/// The CLI now uses exclusively the noteey.com provider. Both `auto`
/// (default) and `provider-noteey` resolve to `ProviderNoteey`; the
/// `--provider` flag is retained only for backward compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ProviderChoice {
    /// Resolve to the noteey.com provider.
    Auto,
    /// The noteey.com provider.
    ProviderNoteey,
}

impl ProviderChoice {
    /// Lowercase kebab-case identifier used in TOML and tracing.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::ProviderNoteey => "provider-noteey",
        }
    }
}

/// Parsed command-line arguments. See `youtube-legend-cli --help` for the
/// rendered help text and the README for the long-form documentation.
#[doc(alias = "Args")]
#[doc(alias = "arguments")]
#[doc(alias = "parser")]
#[doc(alias = "CLI")]
#[doc(alias = "command-line")]
#[doc(alias = "clap")]
#[doc(alias = "argument parser")]
#[derive(Debug, Parser, Clone)]
#[command(
    name = "youtube-legend-cli",
    version,
    about = "Non-interactive Rust CLI that downloads YouTube subtitles via third-party providers using a native Unix stdin/stdout interface.",
    long_about = None,
    propagate_version = true,
    disable_help_subcommand = true,
    after_help = "Examples:\n  youtube-legend-cli https://youtu.be/dQw4w9WgXcQ\n  echo \"https://youtu.be/dQw4w9WgXcQ\" | youtube-legend-cli --format srt\n  cat urls.txt | youtube-legend-cli --batch --json\n  youtube-legend-cli --lang pt --timeout 60 https://youtu.be/dQw4w9WgXcQ",
)]
pub struct Cli {
    /// `YouTube` URL in any of the supported forms (watch, shorts,
    /// embed, youtu.be). Omit when piping a URL through stdin or when
    /// using `--batch`.
    #[arg(
        value_name = "URL",
        help = "YouTube URL (watch, shorts, embed, or youtu.be)"
    )]
    pub url: Option<String>,

    /// Preferred subtitle language. Accepts ISO 639-1 codes or full
    /// BCP 47 locales (`pt-BR`, `en-US`); the primary subtag decides.
    #[arg(
        long,
        value_name = "LANG",
        help = "Preferred subtitle language: ISO 639-1 or BCP 47 (en, pt, pt-BR, es, fr, de, it)",
        default_value = "en",
        value_parser = parse_language
    )]
    pub lang: LanguageArg,

    /// Output format. `txt` strips timestamps; `srt` preserves them
    /// (srt unavailable with provider-noteey).
    #[arg(
        long,
        value_name = "FORMAT",
        help = "Output format: txt (default) or srt (unavailable with provider-noteey)",
        default_value = "txt"
    )]
    pub format: FormatArg,

    /// HTTP request timeout in seconds.
    #[arg(
        long,
        value_name = "SECONDS",
        help = "HTTP request timeout in seconds",
        default_value_t = 30
    )]
    pub timeout: u64,

    /// Emit tracing events at info level to stderr.
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Emit tracing events to stderr"
    )]
    pub verbose: bool,

    /// Suppress all non-error output on stderr.
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Suppress all stderr output except errors"
    )]
    pub quiet: bool,

    /// Path to a TOML config file. When set, flags in the config are
    /// applied first and CLI flags override the config values.
    #[arg(long, value_name = "PATH", help = "Path to a TOML config file")]
    pub config: Option<PathBuf>,

    /// Tracing log level. Falls back to the `RUST_LOG` env var when
    /// unset; default `warn` matches the `tracing-subscriber` baseline.
    #[arg(
        long,
        value_name = "LEVEL",
        help = "Log level: error, warn, info, debug, trace",
        default_value = "warn",
        value_enum
    )]
    pub log_level: LogLevelArg,

    /// Log output format. `json` is suitable for ingestion by log
    /// aggregators; `text` is the human-readable default.
    #[arg(
        long,
        value_name = "FORMAT",
        help = "Log format: text (default) or json",
        default_value = "text",
        value_enum
    )]
    pub log_format: LogFormatArg,

    /// ANSI colour policy. Honours `NO_COLOR` and `CLICOLOR_FORCE`
    /// when set to `auto` (the default).
    #[arg(
        long,
        value_name = "WHEN",
        help = "Colour output: auto, always, never",
        default_value = "auto",
        value_enum
    )]
    pub color: ColorArg,

    /// Disable progress bars and spinners on stderr.
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Suppress progress bars on stderr"
    )]
    pub no_progress: bool,

    /// Run without making any network requests. Reads are served from
    /// the local cache only; writes still update the cache.
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Skip network I/O and serve reads from cache only"
    )]
    pub dry_run: bool,

    /// Assume "yes" for any interactive confirmation prompt.
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Assume yes for any confirmation prompt"
    )]
    pub yes: bool,

    /// Emit a single JSON object to stdout instead of the raw subtitle.
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Emit structured JSON to stdout"
    )]
    pub json: bool,

    /// Read multiple URLs from stdin, one per line, and emit a
    /// concatenated or JSON-per-line result.
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Read multiple URLs from stdin, one per line"
    )]
    pub batch: bool,

    /// Override the User-Agent header used by both providers.
    #[arg(
        long,
        value_name = "STRING",
        help = "Custom User-Agent for HTTP requests"
    )]
    pub user_agent: Option<String>,

    /// Cache TTL in hours. Expired entries are removed on read.
    #[arg(
        long,
        value_name = "HOURS",
        help = "Local cache TTL in hours",
        default_value_t = 24
    )]
    pub cache_ttl: u64,

    /// Skip cache reads (cache writes still happen).
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Disable reads from the local cache"
    )]
    pub no_cache: bool,

    /// Provider selection. Retained for backward compatibility; both
    /// `auto` (default) and `provider-noteey` resolve to the
    /// noteey.com provider.
    #[arg(
        long,
        value_name = "PROVIDER",
        help = "Provider selection: auto, provider-noteey",
        default_value = "auto",
        value_enum
    )]
    pub provider: Option<ProviderChoice>,
}

impl Cli {
    /// Resolve the timeout as a [`Duration`].
    pub fn timeout_duration(&self) -> Duration {
        Duration::from_secs(self.timeout)
    }

    /// Resolve the cache TTL as a [`Duration`].
    pub fn cache_ttl_duration(&self) -> Duration {
        Duration::from_secs(self.cache_ttl * 3600)
    }

    /// User-Agent header value, falling back to the crate default.
    pub fn effective_user_agent(&self) -> String {
        self.user_agent.clone().unwrap_or_else(|| {
            concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")).to_string()
        })
    }

    /// Resolve the effective log level. Honours `--verbose` and
    /// `RUST_LOG` when the flag is at its default value.
    pub fn effective_log_level(&self) -> LogLevelArg {
        if self.log_level != LogLevelArg::Warn {
            return self.log_level;
        }
        // GAP-AUD-2026-066: --verbose bumps to info when --log-level
        // was not explicitly set.
        if self.verbose {
            return LogLevelArg::Info;
        }
        match std::env::var("RUST_LOG").ok().as_deref() {
            Some("error") => LogLevelArg::Error,
            Some("warn") | None => LogLevelArg::Warn,
            Some("info") => LogLevelArg::Info,
            Some("debug") => LogLevelArg::Debug,
            Some("trace") => LogLevelArg::Trace,
            Some(_) => LogLevelArg::Info,
        }
    }

    /// Resolve the effective log format. Defaults to `text`.
    pub fn effective_log_format(&self) -> LogFormatArg {
        self.log_format
    }

    /// Resolve the effective colour policy, honouring `NO_COLOR` and
    /// `CLICOLOR_FORCE` env vars when `--color` is `auto`.
    pub fn effective_color(&self) -> ColorArg {
        match self.color {
            ColorArg::Always => ColorArg::Always,
            ColorArg::Never => ColorArg::Never,
            ColorArg::Auto => {
                if std::env::var_os("NO_COLOR").is_some() {
                    ColorArg::Never
                } else if std::env::var_os("CLICOLOR_FORCE").is_some() {
                    ColorArg::Always
                } else {
                    ColorArg::Auto
                }
            }
        }
    }

    /// Propagate CLI flag values into env vars so that downstream
    /// crates (`tracing`, `colored`, `indicatif`) observe the chosen
    /// configuration. Call this once at the start of `main` before
    /// any subscriber or progress bar is initialised.
    pub fn apply_overrides(&self) {
        let level = self.effective_log_level();
        // `set_var` is safe under edition = "2021"; we set env vars
        // only during the single-threaded early phase of `main`,
        // before any thread is spawned, so concurrent readers are
        // impossible.
        std::env::set_var("YT_LOG_LEVEL", level.as_str());
        let format = self.effective_log_format();
        std::env::set_var("YT_LOG_FORMAT", format.as_str());
        let color = self.effective_color();
        match color {
            ColorArg::Never => std::env::set_var("NO_COLOR", "1"),
            ColorArg::Always => std::env::set_var("CLICOLOR_FORCE", "1"),
            ColorArg::Auto => {}
        }
        if self.no_progress {
            std::env::set_var("YT_NO_PROGRESS", "1");
        }
        if self.dry_run {
            std::env::set_var("YT_DRY_RUN", "1");
        }
    }
}

/// GAP-E2E-016: per-flag "was this set on the command line?" bitmask.
/// Populated by [`parse_with_overrides`] via `ArgMatches::value_source`
/// so the config-file merge can tell apart "user omitted the flag"
/// from "user passed the flag with the built-in default value".
///
/// The previous sentinel logic in `apply_config_overrides` compared
/// the parsed field against its default literal (e.g.
/// `if self.timeout == 30`). That pattern silently mis-applied
/// config overrides when the user passed the flag explicitly with
/// the same value as the default (`--timeout 30` would still get
/// overridden by `timeout = 99` from config). Tracking the source
/// per-field removes the ambiguity.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct CliOverrideFlags {
    /// `true` if `--lang` appeared on the command line.
    pub lang: bool,
    /// `true` if `--format` appeared on the command line.
    pub format: bool,
    /// `true` if `--timeout` appeared on the command line.
    pub timeout: bool,
    /// `true` if `--cache-ttl` appeared on the command line.
    pub cache_ttl: bool,
    /// `true` if `--user-agent` appeared on the command line.
    pub user_agent: bool,
    /// `true` if `--log-level` appeared on the command line.
    pub log_level: bool,
    /// `true` if `--log-format` appeared on the command line.
    pub log_format: bool,
    /// `true` if `--color` appeared on the command line.
    pub color: bool,
    /// `true` if `--provider` appeared on the command line.
    pub provider: bool,
    /// `true` if `--verbose` appeared on the command line.
    pub verbose: bool,
    /// `true` if `--quiet` appeared on the command line.
    pub quiet: bool,
    /// `true` if `--json` appeared on the command line.
    pub json: bool,
    /// `true` if `--batch` appeared on the command line.
    pub batch: bool,
    /// `true` if `--no-cache` appeared on the command line.
    pub no_cache: bool,
    /// `true` if `--dry-run` appeared on the command line.
    pub dry_run: bool,
    /// `true` if `--no-progress` appeared on the command line.
    pub no_progress: bool,
    /// `true` if `--yes` appeared on the command line.
    pub yes: bool,
}

/// Parse the CLI and return both the populated [`Cli`] and a
/// [`CliOverrideFlags`] bitmask describing which flags the user
/// supplied on the command line (vs which the parser filled from
/// the `default_value` directive). Calling this in `main` is
/// strictly equivalent to `Cli::parse()` for the populated `Cli`,
/// but the additional flag tracking is required to make the
/// config↔CLI merge deterministic.
///
/// # Errors
///
/// - Any error `Cli::parse()` would produce: argument parse errors,
///   conflicting flags rejected by `clap`, invalid value parsers.
pub fn parse_with_overrides() -> Result<(Cli, CliOverrideFlags), clap::Error> {
    use clap::parser::ValueSource;

    let cmd = <Cli as clap::CommandFactory>::command();
    // `get_matches_from` consumes its argv argument; we pass a fresh
    // iterator each call so the function can be invoked more than
    // once in the same process (e.g. in tests).
    let matches = cmd.get_matches_from(std::env::args_os());

    // Walk the matches and read the value source for every flag we
    // care about. `Some(CommandLine)` means the operator typed it;
    // `Some(EnvVariable)` means it came from an env var; otherwise
    // the parser filled from `default_value` and we must treat the
    // field as "not set by the user".
    let src = |id: &str| matches.value_source(id) == Some(ValueSource::CommandLine);

    let flags = CliOverrideFlags {
        lang: src("lang"),
        format: src("format"),
        timeout: src("timeout"),
        cache_ttl: src("cache_ttl"),
        user_agent: src("user_agent"),
        log_level: src("log_level"),
        log_format: src("log_format"),
        color: src("color"),
        provider: src("provider"),
        verbose: src("verbose"),
        quiet: src("quiet"),
        json: src("json"),
        batch: src("batch"),
        no_cache: src("no_cache"),
        dry_run: src("dry_run"),
        no_progress: src("no_progress"),
        yes: src("yes"),
    };

    let cli = <Cli as clap::FromArgMatches>::from_arg_matches(&matches)?;
    Ok((cli, flags))
}

impl Cli {
    /// Reject combinations of flags that would be impossible or surprising
    /// to execute. Returns [`AppError::InvalidUsage`] on the first
    /// impossibility.
    ///
    /// GAP-E2E-015: the previous signature returned `Result<(), String>`
    /// and forced every caller to bridge the `String → AppError` gap.
    /// Returning the typed error directly removes the bridge and
    /// keeps the canonic rule "domain functions return
    /// `Result<T, AppError>`".
    ///
    /// # Errors
    ///
    /// Returns [`AppError::InvalidUsage`] when a flag combination is
    /// impossible:
    ///
    /// - `--batch` combined with a positional URL
    /// - no URL, no stdin pipe, and no `--batch`
    pub fn validate(&self) -> AppResult<()> {
        if self.batch && self.url.is_some() {
            return Err(AppError::InvalidUsage(
                "--batch cannot be combined with a positional url".to_string(),
            ));
        }
        if self.url.is_none() && is_stdin_tty_or_blocked() && !self.batch {
            return Err(AppError::InvalidUsage(
                "no url provided; pass a positional url, pipe through stdin, or use --batch"
                    .to_string(),
            ));
        }
        if self.url.as_ref().is_some_and(|u| u.len() > 2048) {
            return Err(AppError::InvalidUsage(
                "url exceeds 2048 characters".to_string(),
            ));
        }
        if self.quiet && self.verbose {
            return Err(AppError::InvalidUsage(
                "--quiet cannot be combined with --verbose".to_string(),
            ));
        }
        if self.timeout == 0 {
            return Err(AppError::InvalidUsage(
                "--timeout must be greater than zero".to_string(),
            ));
        }
        if self.cache_ttl == 0 {
            return Err(AppError::InvalidUsage(
                "--cache-ttl must be greater than zero".to_string(),
            ));
        }
        if self.dry_run && self.batch {
            return Err(AppError::InvalidUsage(
                "--dry-run cannot be combined with --batch".to_string(),
            ));
        }
        Ok(())
    }
}

/// Parse an ISO 639-1 code or a full BCP 47 locale into a
/// [`LanguageArg`] (FR-009). Normalisation: trim, drop an encoding
/// suffix such as `.UTF-8`, treat `_` and `-` as equivalent, and
/// lowercase the primary subtag before matching.
fn parse_language(raw: &str) -> Result<LanguageArg, String> {
    let cleaned = raw
        .trim()
        .split('.')
        .next()
        .unwrap_or_default()
        .replace('_', "-");
    let primary = cleaned
        .split('-')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match primary.as_str() {
        "en" => Ok(LanguageArg::En),
        "pt" => Ok(LanguageArg::Pt),
        "es" => Ok(LanguageArg::Es),
        "fr" => Ok(LanguageArg::Fr),
        "de" => Ok(LanguageArg::De),
        "it" => Ok(LanguageArg::It),
        _ => Err(format!(
            "invalid locale: {raw} (supported: en, pt, es, fr, de, it, or BCP 47 forms such as pt-BR)"
        )),
    }
}

/// Load CLI defaults from a TOML config file. The file is a flat
/// table whose keys mirror the long-form CLI flag names (without the
/// leading `--`). Unknown keys are rejected with a clear error
/// message so typos surface immediately. CLI flags always override
/// config values, so the precedence is CLI > config > built-in
/// default.
///
/// Supported keys: `url`, `lang`, `format`, `timeout`, `cache_ttl`,
/// `user_agent`. Booleans `verbose`, `quiet`, `json`, `batch`,
/// `no_cache`, `dry_run`, `no_progress`, `yes`. Optional `log_level`,
/// `log_format`, `color`.
///
/// # Errors
///
/// - [`crate::error::AppError::Config`] when the file is missing,
///   unreadable, or contains malformed TOML or an unknown key. This
///   maps to sysexits `EX_CONFIG = 78`, distinct from
///   `AppError::InvalidUsage` (exit 64) which is reserved for
///   post-parse CLI argument validation failures.
pub fn load_config(path: &std::path::Path) -> AppResult<ConfigOverrides> {
    use crate::error::AppError;
    use std::fs;

    let text = fs::read_to_string(path).map_err(|e| {
        AppError::Config(format!(
            "could not read config file {}: {e}",
            path.display()
        ))
    })?;
    let table: toml::Table = text.parse().map_err(|e| {
        AppError::Config(format!(
            "config file {} is not valid TOML: {e}",
            path.display()
        ))
    })?;

    let mut out = ConfigOverrides::default();
    for (key, value) in &table {
        match key.as_str() {
            "url" => {
                out.url = Some(
                    value
                        .as_str()
                        .ok_or_else(|| invalid_type(key, "string"))?
                        .to_string(),
                )
            }
            "lang" => {
                let raw = value.as_str().ok_or_else(|| invalid_type(key, "string"))?;
                out.lang = Some(parse_language(raw).map_err(AppError::Config)?);
            }
            "format" => {
                let raw = value.as_str().ok_or_else(|| invalid_type(key, "string"))?;
                out.format = Some(match raw {
                    "txt" => FormatArg::Txt,
                    "srt" => FormatArg::Srt,
                    other => {
                        return Err(AppError::Config(format!(
                            "config: invalid format `{other}` (expected txt or srt)"
                        )))
                    }
                });
            }
            "timeout" => {
                out.timeout = Some(
                    value
                        .as_integer()
                        .ok_or_else(|| invalid_type(key, "integer"))? as u64,
                )
            }
            "cache_ttl" => {
                out.cache_ttl = Some(
                    value
                        .as_integer()
                        .ok_or_else(|| invalid_type(key, "integer"))? as u64,
                )
            }
            "user_agent" => {
                out.user_agent = Some(
                    value
                        .as_str()
                        .ok_or_else(|| invalid_type(key, "string"))?
                        .to_string(),
                )
            }
            "verbose" => {
                out.verbose = Some(
                    value
                        .as_bool()
                        .ok_or_else(|| invalid_type(key, "boolean"))?,
                )
            }
            "quiet" => {
                out.quiet = Some(
                    value
                        .as_bool()
                        .ok_or_else(|| invalid_type(key, "boolean"))?,
                )
            }
            "json" => {
                out.json = Some(
                    value
                        .as_bool()
                        .ok_or_else(|| invalid_type(key, "boolean"))?,
                )
            }
            "batch" => {
                out.batch = Some(
                    value
                        .as_bool()
                        .ok_or_else(|| invalid_type(key, "boolean"))?,
                )
            }
            "no_cache" => {
                out.no_cache = Some(
                    value
                        .as_bool()
                        .ok_or_else(|| invalid_type(key, "boolean"))?,
                )
            }
            "dry_run" => {
                out.dry_run = Some(
                    value
                        .as_bool()
                        .ok_or_else(|| invalid_type(key, "boolean"))?,
                )
            }
            "no_progress" => {
                out.no_progress = Some(
                    value
                        .as_bool()
                        .ok_or_else(|| invalid_type(key, "boolean"))?,
                )
            }
            "yes" => {
                out.yes = Some(
                    value
                        .as_bool()
                        .ok_or_else(|| invalid_type(key, "boolean"))?,
                )
            }
            "provider" => {
                let raw = value.as_str().ok_or_else(|| invalid_type(key, "string"))?;
                out.provider = Some(match raw {
                    "auto" => ProviderChoice::Auto,
                    "provider-noteey" => ProviderChoice::ProviderNoteey,
                    other => {
                        return Err(AppError::Config(format!(
                            "config: invalid provider `{other}` (expected auto|provider-noteey)"
                        )))
                    }
                });
            }
            "log_level" => {
                let raw = value.as_str().ok_or_else(|| invalid_type(key, "string"))?;
                out.log_level = Some(match raw {
                    "error" => LogLevelArg::Error,
                    "warn" => LogLevelArg::Warn,
                    "info" => LogLevelArg::Info,
                    "debug" => LogLevelArg::Debug,
                    "trace" => LogLevelArg::Trace,
                    other => {
                        return Err(AppError::Config(format!(
                        "config: invalid log_level `{other}` (expected error|warn|info|debug|trace)"
                    )))
                    }
                });
            }
            "log_format" => {
                let raw = value.as_str().ok_or_else(|| invalid_type(key, "string"))?;
                out.log_format = Some(match raw {
                    "text" => LogFormatArg::Text,
                    "json" => LogFormatArg::Json,
                    other => {
                        return Err(AppError::Config(format!(
                            "config: invalid log_format `{other}` (expected text or json)"
                        )))
                    }
                });
            }
            "color" => {
                let raw = value.as_str().ok_or_else(|| invalid_type(key, "string"))?;
                out.color = Some(match raw {
                    "auto" => ColorArg::Auto,
                    "always" => ColorArg::Always,
                    "never" => ColorArg::Never,
                    other => {
                        return Err(AppError::Config(format!(
                            "config: invalid color `{other}` (expected auto|always|never)"
                        )))
                    }
                });
            }
            other => {
                return Err(AppError::Config(format!("config: unknown key `{other}`")));
            }
        }
    }
    Ok(out)
}

fn invalid_type(key: &str, expected: &str) -> crate::error::AppError {
    crate::error::AppError::Config(format!(
        "config: key `{key}` has wrong type (expected {expected})"
    ))
}

/// Field-level overrides loaded from a TOML config file. Each
/// `Option<T>` is `Some` only when the user actually set the key;
/// `None` means "use the built-in default". This shape lets
/// `Cli::apply_config_overrides` distinguish "user set this in
/// config" from "user did not set it" without sentinel values.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
#[allow(missing_docs)]
pub struct ConfigOverrides {
    pub url: Option<String>,
    pub lang: Option<LanguageArg>,
    pub format: Option<FormatArg>,
    pub timeout: Option<u64>,
    pub cache_ttl: Option<u64>,
    pub user_agent: Option<String>,
    pub verbose: Option<bool>,
    pub quiet: Option<bool>,
    pub json: Option<bool>,
    pub batch: Option<bool>,
    pub no_cache: Option<bool>,
    pub dry_run: Option<bool>,
    pub no_progress: Option<bool>,
    pub yes: Option<bool>,
    pub log_level: Option<LogLevelArg>,
    pub log_format: Option<LogFormatArg>,
    pub color: Option<ColorArg>,
    pub provider: Option<ProviderChoice>,
}

impl Cli {
    /// Merge config-file overrides into a freshly-parsed Cli. CLI
    /// flags always win: a flag the operator set on the command line
    /// (recorded in [`CliOverrideFlags`]) is left alone, and only
    /// flags the operator omitted are replaced with the config-file
    /// value when present.
    ///
    /// GAP-E2E-016: the previous implementation compared each field
    /// against its default literal (`if self.timeout == 30`) to
    /// detect "operator omitted the flag". That sentinel silently
    /// mis-applied config overrides when the operator typed the flag
    /// explicitly with the same value as the default
    /// (`--timeout 30` would still get overridden by `timeout = 99`
    /// from config). `flags` is the only reliable source for the
    /// "did the operator set this?" answer.
    pub fn apply_config_overrides(&mut self, cfg: ConfigOverrides, flags: &CliOverrideFlags) {
        if !flags.lang {
            if let Some(l) = cfg.lang {
                self.lang = l;
            }
        }
        if !flags.format {
            if let Some(f) = cfg.format {
                self.format = f;
            }
        }
        if !flags.timeout {
            if let Some(t) = cfg.timeout {
                self.timeout = t;
            }
        }
        if !flags.cache_ttl {
            if let Some(t) = cfg.cache_ttl {
                self.cache_ttl = t;
            }
        }
        if !flags.user_agent {
            self.user_agent = cfg.user_agent;
        }
        if !flags.verbose {
            if let Some(v) = cfg.verbose {
                self.verbose = v;
            }
        }
        if !flags.quiet {
            if let Some(q) = cfg.quiet {
                self.quiet = q;
            }
        }
        if !flags.json {
            if let Some(j) = cfg.json {
                self.json = j;
            }
        }
        if !flags.batch {
            if let Some(b) = cfg.batch {
                self.batch = b;
            }
        }
        if !flags.no_cache {
            if let Some(n) = cfg.no_cache {
                self.no_cache = n;
            }
        }
        if !flags.dry_run {
            if let Some(d) = cfg.dry_run {
                self.dry_run = d;
            }
        }
        if !flags.no_progress {
            if let Some(n) = cfg.no_progress {
                self.no_progress = n;
            }
        }
        if !flags.yes {
            if let Some(y) = cfg.yes {
                self.yes = y;
            }
        }
        if !flags.log_level {
            if let Some(l) = cfg.log_level {
                self.log_level = l;
            }
        }
        if !flags.log_format {
            if let Some(f) = cfg.log_format {
                self.log_format = f;
            }
        }
        if !flags.color {
            if let Some(c) = cfg.color {
                self.color = c;
            }
        }
        if !flags.provider {
            self.provider = cfg.provider;
        }
        // `url` is positional and has no `default_value`, so it is
        // always considered "operator omitted" when `None`. We keep
        // the same behaviour as before.
        if self.url.is_none() {
            self.url = cfg.url;
        }
    }
}

fn is_stdin_tty_or_blocked() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, FormatArg, LanguageArg};
    use crate::cli::{ColorArg, LogFormatArg, LogLevelArg};
    use crate::error::AppError;
    use clap::Parser;

    /// Test-only helper: drive `parse_with_overrides` from a
    /// controllable argv slice instead of `std::env::args_os`.
    /// Returns `(Cli, CliOverrideFlags)` for assertion.
    fn parse_with_overrides_from<const N: usize>(args: [&str; N]) -> (Cli, CliOverrideFlags) {
        let cmd = <Cli as clap::CommandFactory>::command();
        let matches = cmd.get_matches_from(args);
        use clap::parser::ValueSource;
        let src = |id: &str| matches.value_source(id) == Some(ValueSource::CommandLine);
        let flags = CliOverrideFlags {
            lang: src("lang"),
            format: src("format"),
            timeout: src("timeout"),
            cache_ttl: src("cache_ttl"),
            user_agent: src("user_agent"),
            log_level: src("log_level"),
            log_format: src("log_format"),
            color: src("color"),
            provider: src("provider"),
            verbose: src("verbose"),
            quiet: src("quiet"),
            json: src("json"),
            batch: src("batch"),
            no_cache: src("no_cache"),
            dry_run: src("dry_run"),
            no_progress: src("no_progress"),
            yes: src("yes"),
        };
        let cli =
            <Cli as clap::FromArgMatches>::from_arg_matches(&matches).expect("test argv is valid");
        (cli, flags)
    }

    fn make_cli(url: Option<&str>, batch: bool) -> Cli {
        let mut args = vec!["youtube-legend-cli".to_string()];
        if let Some(u) = url {
            args.push(u.to_string());
        }
        if batch {
            args.push("--batch".to_string());
        }
        Cli::parse_from(args)
    }

    #[test]
    fn validate_accepts_url_only() {
        let cli = make_cli(Some("https://youtu.be/dQw4w9WgXcQ"), false);
        assert!(cli.validate().is_ok());
    }

    #[test]
    fn clap_rejects_invalid_language_via_try_parse_from() {
        // GAP-AUD-002: --lang xx exits with code 2 via clap::Error::exit()
        // before reaching AppError. try_parse_from returns Err so we can
        // assert the type without spawning a process. The exit code 2 is
        // produced by clap::Error::exit() in src/main.rs.
        use clap::Parser;
        let result = Cli::try_parse_from([
            "youtube-legend-cli",
            "--lang",
            "xx",
            "https://youtu.be/dQw4w9WgXcQ",
        ]);
        assert!(
            result.is_err(),
            "clap must reject --lang xx before reaching AppError"
        );
        let err = result.unwrap_err();
        // clap v4 reports rejected enum values as `ValueValidation`
        // (a sub-kind of argument validation), not the legacy
        // `InvalidValue`. Either way, the kind is a parse-time error
        // — never reaching `AppError::exit_code()` — and the process
        // exits with code 2 via `clap::Error::exit()`.
        assert!(matches!(
            err.kind(),
            clap::error::ErrorKind::ValueValidation | clap::error::ErrorKind::InvalidValue
        ));
    }

    #[test]
    fn validate_accepts_batch_with_stdin() {
        let cli = make_cli(None, true);
        assert!(cli.validate().is_ok());
    }

    #[test]
    fn validate_rejects_url_and_batch_together() {
        let cli = make_cli(Some("https://youtu.be/dQw4w9WgXcQ"), true);
        let err = cli.validate().unwrap_err();
        assert!(matches!(err, AppError::InvalidUsage(_)));
        assert!(err.to_string().contains("--batch cannot be combined"));
    }

    #[test]
    fn validate_rejects_url_too_long() {
        let long = "a".repeat(2049);
        let cli = make_cli(Some(&long), false);
        let err = cli.validate().unwrap_err();
        assert!(matches!(err, AppError::InvalidUsage(_)));
        assert!(err.to_string().contains("exceeds 2048"));
    }

    #[test]
    fn validate_rejects_quiet_with_verbose() {
        let cli = Cli::parse_from([
            "youtube-legend-cli",
            "https://youtu.be/dQw4w9WgXcQ",
            "--quiet",
            "--verbose",
        ]);
        let err = cli.validate().unwrap_err();
        assert!(matches!(err, AppError::InvalidUsage(_)));
        assert!(err.to_string().contains("--quiet"));
    }

    #[test]
    fn validate_rejects_zero_timeout() {
        let cli = Cli::parse_from([
            "youtube-legend-cli",
            "https://youtu.be/dQw4w9WgXcQ",
            "--timeout",
            "0",
        ]);
        let err = cli.validate().unwrap_err();
        assert!(matches!(err, AppError::InvalidUsage(_)));
        assert!(err.to_string().contains("--timeout"));
    }

    #[test]
    fn validate_rejects_zero_cache_ttl() {
        let cli = Cli::parse_from([
            "youtube-legend-cli",
            "https://youtu.be/dQw4w9WgXcQ",
            "--cache-ttl",
            "0",
        ]);
        let err = cli.validate().unwrap_err();
        assert!(matches!(err, AppError::InvalidUsage(_)));
        assert!(err.to_string().contains("--cache-ttl"));
    }

    #[test]
    fn validate_accepts_stdin_pipe_path_semantically() {
        let cli = make_cli(None, false);
        let res = cli.validate();
        let is_tty = is_stdin_tty_or_blocked();
        if is_tty {
            assert!(matches!(res, Err(AppError::InvalidUsage(_))));
        } else {
            assert!(res.is_ok());
        }
    }

    #[test]
    fn parse_language_accepts_bcp47_locales() {
        assert_eq!(parse_language("pt-BR"), Ok(LanguageArg::Pt));
        assert_eq!(parse_language("pt_BR.UTF-8"), Ok(LanguageArg::Pt));
        assert_eq!(parse_language("EN-us"), Ok(LanguageArg::En));
        assert_eq!(parse_language("es-AR"), Ok(LanguageArg::Es));
    }

    #[test]
    fn parse_language_rejects_unknown_locale() {
        let err = parse_language("xx-YY").unwrap_err();
        assert!(err.contains("invalid locale: xx-YY"));
    }

    #[test]
    fn lang_flag_accepts_bcp47_from_argv() {
        let cli = Cli::parse_from([
            "youtube-legend-cli",
            "https://youtu.be/dQw4w9WgXcQ",
            "--lang",
            "pt-BR",
        ]);
        assert_eq!(cli.lang, LanguageArg::Pt);
    }

    #[test]
    fn language_arg_maps_to_iso_codes() {
        assert_eq!(format_lang(LanguageArg::En), "en");
        assert_eq!(format_lang(LanguageArg::Pt), "pt");
        assert_eq!(format_lang(LanguageArg::Es), "es");
    }

    #[test]
    fn format_arg_maps_to_extensions() {
        assert_eq!(format_fmt(FormatArg::Txt), "txt");
        assert_eq!(format_fmt(FormatArg::Srt), "srt");
    }

    fn format_lang(l: LanguageArg) -> &'static str {
        match l {
            LanguageArg::En => "en",
            LanguageArg::Pt => "pt",
            LanguageArg::Es => "es",
            LanguageArg::Fr => "fr",
            LanguageArg::De => "de",
            LanguageArg::It => "it",
        }
    }

    #[test]
    fn cli_accepts_all_global_flags() {
        let cli = Cli::parse_from([
            "youtube-legend-cli",
            "https://youtu.be/dQw4w9WgXcQ",
            "--config",
            "/tmp/cfg.toml",
            "--log-level",
            "debug",
            "--log-format",
            "json",
            "--color",
            "never",
            "--no-progress",
            "--dry-run",
            "--yes",
        ]);
        assert_eq!(cli.log_level, LogLevelArg::Debug);
        assert_eq!(cli.log_format, LogFormatArg::Json);
        assert_eq!(cli.color, ColorArg::Never);
        assert!(cli.no_progress);
        assert!(cli.dry_run);
        assert!(cli.yes);
        assert_eq!(cli.config, Some(std::path::PathBuf::from("/tmp/cfg.toml")));
    }

    #[test]
    fn log_level_enum_maps_to_tracing() {
        assert_eq!(LogLevelArg::Error.as_str(), "error");
        assert_eq!(LogLevelArg::Warn.as_str(), "warn");
        assert_eq!(LogLevelArg::Info.as_str(), "info");
        assert_eq!(LogLevelArg::Debug.as_str(), "debug");
        assert_eq!(LogLevelArg::Trace.as_str(), "trace");
    }

    #[test]
    fn color_env_var_overrides_default() {
        let cli = Cli::parse_from(["youtube-legend-cli", "https://youtu.be/dQw4w9WgXcQ"]);
        assert_eq!(cli.color, ColorArg::Auto);
        // The default does not set NO_COLOR/CLICOLOR_FORCE; we only
        // assert that the public API resolves to Auto without panic.
        let _ = cli.effective_color();
    }

    #[test]
    fn dry_run_rejects_batch() {
        let cli = Cli::parse_from(["youtube-legend-cli", "--dry-run", "--batch"]);
        let err = cli.validate().unwrap_err();
        assert!(matches!(err, AppError::InvalidUsage(_)));
        assert!(err.to_string().contains("--dry-run"));
        assert!(err.to_string().contains("--batch"));
    }

    #[test]
    fn apply_overrides_sets_env_vars() {
        // Snapshot env vars that we touch so we can restore them.
        let prev_level = std::env::var("YT_LOG_LEVEL").ok();
        let prev_format = std::env::var("YT_LOG_FORMAT").ok();
        let prev_no_color = std::env::var("NO_COLOR").ok();
        let prev_force = std::env::var("CLICOLOR_FORCE").ok();
        let prev_dry = std::env::var("YT_DRY_RUN").ok();
        let prev_progress = std::env::var("YT_NO_PROGRESS").ok();

        let cli = Cli::parse_from([
            "youtube-legend-cli",
            "https://youtu.be/dQw4w9WgXcQ",
            "--log-level",
            "trace",
            "--log-format",
            "json",
            "--color",
            "never",
            "--no-progress",
            "--dry-run",
        ]);
        cli.apply_overrides();

        assert_eq!(std::env::var("YT_LOG_LEVEL").ok().as_deref(), Some("trace"));
        assert_eq!(std::env::var("YT_LOG_FORMAT").ok().as_deref(), Some("json"));
        assert_eq!(std::env::var("NO_COLOR").ok().as_deref(), Some("1"));
        assert_eq!(std::env::var("YT_DRY_RUN").ok().as_deref(), Some("1"));
        assert_eq!(std::env::var("YT_NO_PROGRESS").ok().as_deref(), Some("1"));

        // Restore previous env state to avoid leaking into other tests.
        restore("YT_LOG_LEVEL", prev_level);
        restore("YT_LOG_FORMAT", prev_format);
        restore("NO_COLOR", prev_no_color);
        restore("CLICOLOR_FORCE", prev_force);
        restore("YT_DRY_RUN", prev_dry);
        restore("YT_NO_PROGRESS", prev_progress);
    }

    fn restore(key: &str, prev: Option<String>) {
        match prev {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }

    #[test]
    fn load_config_reads_valid_toml() {
        let dir = std::env::temp_dir();
        let path = dir.join("yt_legend_config_test_valid.toml");
        std::fs::write(
            &path,
            r#"
url = "https://youtu.be/dQw4w9WgXcQ"
lang = "pt"
timeout = 12
cache_ttl = 6
verbose = true
json = false
dry_run = true
log_level = "debug"
log_format = "json"
color = "never"
"#,
        )
        .expect("write tmp config");
        let cfg = load_config(&path).expect("load config");
        assert_eq!(cfg.url.as_deref(), Some("https://youtu.be/dQw4w9WgXcQ"));
        assert!(matches!(cfg.lang, Some(LanguageArg::Pt)));
        assert_eq!(cfg.timeout, Some(12));
        assert_eq!(cfg.cache_ttl, Some(6));
        assert_eq!(cfg.verbose, Some(true));
        assert_eq!(cfg.dry_run, Some(true));
        assert!(matches!(cfg.log_level, Some(LogLevelArg::Debug)));
        assert!(matches!(cfg.log_format, Some(LogFormatArg::Json)));
        assert!(matches!(cfg.color, Some(ColorArg::Never)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn load_config_rejects_invalid_toml() {
        let dir = std::env::temp_dir();
        let path = dir.join("yt_legend_config_test_bad.toml");
        std::fs::write(&path, "this is not = toml [[[").expect("write tmp");
        let err = load_config(&path).unwrap_err();
        assert!(matches!(err, AppError::Config(_)));
        assert_eq!(err.exit_code(), 78);
        let msg = err.to_string();
        assert!(msg.contains("not valid TOML"), "actual: {msg}");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn load_config_rejects_unknown_key() {
        let dir = std::env::temp_dir();
        let path = dir.join("yt_legend_config_test_unknown.toml");
        std::fs::write(&path, "definitely_not_a_flag = 1\n").expect("write tmp");
        let err = load_config(&path).unwrap_err();
        assert!(matches!(err, AppError::Config(_)));
        assert_eq!(err.exit_code(), 78);
        let msg = err.to_string();
        assert!(
            msg.contains("unknown key `definitely_not_a_flag`"),
            "actual: {msg}"
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn load_config_rejects_missing_file() {
        let path = std::path::Path::new("/nonexistent/path/yt_legend.toml");
        let err = load_config(path).unwrap_err();
        assert!(matches!(err, AppError::Config(_)));
        assert_eq!(err.exit_code(), 78);
    }

    #[test]
    fn apply_config_overrides_cli_wins() {
        let (mut cli, flags) = parse_with_overrides_from([
            "youtube-legend-cli",
            "https://youtu.be/from_cli",
            "--lang",
            "es",
        ]);
        let cfg = ConfigOverrides {
            url: Some("https://youtu.be/from_config".to_string()),
            lang: Some(LanguageArg::Pt),
            timeout: Some(99),
            ..Default::default()
        };
        cli.apply_config_overrides(cfg, &flags);
        // CLI wins on url (positional, not defaultable) and lang
        // (operator passed `--lang es` so `flags.lang` is true).
        assert_eq!(cli.url.as_deref(), Some("https://youtu.be/from_cli"));
        assert!(matches!(cli.lang, LanguageArg::Es));
        // Config fills in defaults (timeout was omitted on CLI,
        // `flags.timeout` is false, so the override applies).
        assert_eq!(cli.timeout, 99);
    }

    #[test]
    fn apply_config_overrides_config_fills_defaults() {
        let (mut cli, flags) = parse_with_overrides_from(["youtube-legend-cli"]);
        let cfg = ConfigOverrides {
            timeout: Some(45),
            cache_ttl: Some(2),
            log_level: Some(LogLevelArg::Trace),
            color: Some(ColorArg::Always),
            ..Default::default()
        };
        cli.apply_config_overrides(cfg, &flags);
        assert_eq!(cli.timeout, 45);
        assert_eq!(cli.cache_ttl, 2);
        assert!(matches!(cli.log_level, LogLevelArg::Trace));
        assert!(matches!(cli.color, ColorArg::Always));
    }

    /// GAP-E2E-016 regression: when the operator types a flag
    /// explicitly with the same value as the built-in default
    /// (`--timeout 30` for example), the previous sentinel logic
    /// `if self.timeout == 30` would mis-classify that field as
    /// "operator omitted" and let the config override win. The
    /// `CliOverrideFlags` bitmask avoids the ambiguity.
    #[test]
    fn apply_config_overrides_explicit_default_does_not_get_overridden() {
        let (mut cli, flags) = parse_with_overrides_from(["youtube-legend-cli", "--timeout", "30"]);
        let cfg = ConfigOverrides {
            timeout: Some(99),
            ..Default::default()
        };
        cli.apply_config_overrides(cfg, &flags);
        assert!(
            flags.timeout,
            "flags.timeout must report explicit CLI usage"
        );
        // The explicit `--timeout 30` MUST survive the merge even
        // though its value matches the built-in default.
        assert_eq!(
            cli.timeout, 30,
            "explicit CLI default value must NOT be overridden by config"
        );
    }

    fn format_fmt(f: FormatArg) -> &'static str {
        match f {
            FormatArg::Txt => "txt",
            FormatArg::Srt => "srt",
        }
    }

    #[test]
    fn provider_choice_parses_all_variants() {
        use crate::cli::ProviderChoice;
        let cases = [
            ("auto", ProviderChoice::Auto),
            ("provider-noteey", ProviderChoice::ProviderNoteey),
        ];
        for (flag, expected) in cases {
            let cli = Cli::parse_from([
                "youtube-legend-cli",
                "https://youtu.be/dQw4w9WgXcQ",
                "--provider",
                flag,
            ]);
            assert_eq!(cli.provider, Some(expected), "failed for {flag}");
        }
    }
}
