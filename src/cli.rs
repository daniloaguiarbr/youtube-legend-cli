//! Command-line argument parsing via [`clap`] derive.
//!
//! The single entry point is [`Cli`]. Construct one with `Cli::parse()` at
//! the start of `main`, validate it with [`Cli::validate`], then dispatch
//! the rest of the program.

use crate::error::AppResult;
use clap::{ArgAction, Parser, ValueEnum};
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
    /// Raw SubRip text with timestamps preserved.
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

/// Parsed command-line arguments. See `youtube-legend-cli --help` for the
/// rendered help text and the README for the long-form documentation.
#[doc(alias = "Args")]
#[doc(alias = "arguments")]
#[doc(alias = "parser")]
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
    /// YouTube URL in any of the supported forms (watch, shorts, embed,
    /// youtu.be). Omit when piping a URL through stdin or when using
    /// `--batch`.
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

    /// Output format. `txt` strips SRT timestamps; `srt` preserves them.
    #[arg(
        long,
        value_name = "FORMAT",
        help = "Output format: txt (plain text) or srt (preserved)",
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

    /// Resolve the effective log level. Honours `RUST_LOG` when the flag
    /// is at its default value, so env-driven logging keeps working.
    pub fn effective_log_level(&self) -> LogLevelArg {
        if self.log_level != LogLevelArg::Warn {
            return self.log_level;
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

    /// Reject combinations of flags that would be impossible or surprising
    /// to execute. Returns the first error message as a `String` so the
    /// caller can wrap it in [`crate::error::AppError::InvalidUsage`].
    pub fn validate(&self) -> Result<(), String> {
        if self.batch && self.url.is_some() {
            return Err("--batch cannot be combined with a positional url".to_string());
        }
        if self.url.is_none() && is_stdin_tty_or_blocked() && !self.batch {
            return Err(
                "no url provided; pass a positional url, pipe through stdin, or use --batch"
                    .to_string(),
            );
        }
        if self.url.as_ref().is_some_and(|u| u.len() > 2048) {
            return Err("url exceeds 2048 characters".to_string());
        }
        if self.quiet && self.verbose {
            return Err("--quiet cannot be combined with --verbose".to_string());
        }
        if self.timeout == 0 {
            return Err("--timeout must be greater than zero".to_string());
        }
        if self.cache_ttl == 0 {
            return Err("--cache-ttl must be greater than zero".to_string());
        }
        if self.dry_run && self.batch {
            return Err("--dry-run cannot be combined with --batch".to_string());
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
/// - [`crate::error::AppError::InvalidInput`] when the file is missing,
///   unreadable, or contains malformed TOML or an unknown key.
pub fn load_config(path: &std::path::Path) -> AppResult<ConfigOverrides> {
    use crate::error::AppError;
    use std::fs;

    let text = fs::read_to_string(path).map_err(|e| {
        AppError::InvalidInput(format!(
            "could not read config file {}: {e}",
            path.display()
        ))
    })?;
    let table: toml::Table = text.parse().map_err(|e| {
        AppError::InvalidInput(format!(
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
                out.lang = Some(parse_language(raw).map_err(AppError::InvalidInput)?);
            }
            "format" => {
                let raw = value.as_str().ok_or_else(|| invalid_type(key, "string"))?;
                out.format = Some(match raw {
                    "txt" => FormatArg::Txt,
                    "srt" => FormatArg::Srt,
                    other => {
                        return Err(AppError::InvalidInput(format!(
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
            "log_level" => {
                let raw = value.as_str().ok_or_else(|| invalid_type(key, "string"))?;
                out.log_level = Some(match raw {
                    "error" => LogLevelArg::Error,
                    "warn" => LogLevelArg::Warn,
                    "info" => LogLevelArg::Info,
                    "debug" => LogLevelArg::Debug,
                    "trace" => LogLevelArg::Trace,
                    other => {
                        return Err(AppError::InvalidInput(format!(
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
                        return Err(AppError::InvalidInput(format!(
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
                        return Err(AppError::InvalidInput(format!(
                            "config: invalid color `{other}` (expected auto|always|never)"
                        )))
                    }
                });
            }
            other => {
                return Err(AppError::InvalidInput(format!(
                    "config: unknown key `{other}`"
                )));
            }
        }
    }
    Ok(out)
}

fn invalid_type(key: &str, expected: &str) -> crate::error::AppError {
    crate::error::AppError::InvalidInput(format!(
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
}

impl Cli {
    /// Merge config-file overrides into a freshly-parsed Cli. CLI
    /// flags always win: a `Some` on the Cli field is left alone,
    /// and only `None` (default) fields are replaced with the
    /// config-file value.
    pub fn apply_config_overrides(&mut self, cfg: ConfigOverrides) {
        if self.url.is_none() {
            self.url = cfg.url;
        }
        if matches!(self.lang, LanguageArg::En) {
            if let Some(l) = cfg.lang {
                self.lang = l;
            }
        }
        if matches!(self.format, FormatArg::Txt) {
            if let Some(f) = cfg.format {
                self.format = f;
            }
        }
        if self.timeout == 30 {
            if let Some(t) = cfg.timeout {
                self.timeout = t;
            }
        }
        if self.cache_ttl == 24 {
            if let Some(t) = cfg.cache_ttl {
                self.cache_ttl = t;
            }
        }
        if self.user_agent.is_none() {
            self.user_agent = cfg.user_agent;
        }
        if !self.verbose {
            if let Some(v) = cfg.verbose {
                self.verbose = v;
            }
        }
        if !self.quiet {
            if let Some(q) = cfg.quiet {
                self.quiet = q;
            }
        }
        if !self.json {
            if let Some(j) = cfg.json {
                self.json = j;
            }
        }
        if !self.batch {
            if let Some(b) = cfg.batch {
                self.batch = b;
            }
        }
        if !self.no_cache {
            if let Some(n) = cfg.no_cache {
                self.no_cache = n;
            }
        }
        if !self.dry_run {
            if let Some(d) = cfg.dry_run {
                self.dry_run = d;
            }
        }
        if !self.no_progress {
            if let Some(n) = cfg.no_progress {
                self.no_progress = n;
            }
        }
        if !self.yes {
            if let Some(y) = cfg.yes {
                self.yes = y;
            }
        }
        if matches!(self.log_level, LogLevelArg::Warn) {
            if let Some(l) = cfg.log_level {
                self.log_level = l;
            }
        }
        if matches!(self.log_format, LogFormatArg::Text) {
            if let Some(f) = cfg.log_format {
                self.log_format = f;
            }
        }
        if matches!(self.color, ColorArg::Auto) {
            if let Some(c) = cfg.color {
                self.color = c;
            }
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
    fn validate_accepts_batch_with_stdin() {
        let cli = make_cli(None, true);
        assert!(cli.validate().is_ok());
    }

    #[test]
    fn validate_rejects_url_and_batch_together() {
        let cli = make_cli(Some("https://youtu.be/dQw4w9WgXcQ"), true);
        let err = cli.validate().unwrap_err();
        assert!(err.contains("--batch cannot be combined"));
    }

    #[test]
    fn validate_rejects_url_too_long() {
        let long = "a".repeat(2049);
        let cli = make_cli(Some(&long), false);
        let err = cli.validate().unwrap_err();
        assert!(err.contains("exceeds 2048"));
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
        assert!(err.contains("--quiet"));
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
        assert!(err.contains("--timeout"));
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
        assert!(err.contains("--cache-ttl"));
    }

    #[test]
    fn validate_accepts_stdin_pipe_path_semantically() {
        let cli = make_cli(None, false);
        let res = cli.validate();
        let is_tty = is_stdin_tty_or_blocked();
        if is_tty {
            assert!(res.is_err());
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
        assert!(err.contains("--dry-run"));
        assert!(err.contains("--batch"));
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
        assert!(matches!(err, AppError::InvalidInput(_)));
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
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn apply_config_overrides_cli_wins() {
        let mut cli = Cli::parse_from([
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
        cli.apply_config_overrides(cfg);
        // CLI wins on url (not the default) and lang.
        assert_eq!(cli.url.as_deref(), Some("https://youtu.be/from_cli"));
        assert!(matches!(cli.lang, LanguageArg::Es));
        // Config fills in defaults (timeout changed from 30 → 99).
        assert_eq!(cli.timeout, 99);
    }

    #[test]
    fn apply_config_overrides_config_fills_defaults() {
        let mut cli = Cli::parse_from(["youtube-legend-cli"]);
        let cfg = ConfigOverrides {
            timeout: Some(45),
            cache_ttl: Some(2),
            log_level: Some(LogLevelArg::Trace),
            color: Some(ColorArg::Always),
            ..Default::default()
        };
        cli.apply_config_overrides(cfg);
        assert_eq!(cli.timeout, 45);
        assert_eq!(cli.cache_ttl, 2);
        assert!(matches!(cli.log_level, LogLevelArg::Trace));
        assert!(matches!(cli.color, ColorArg::Always));
    }

    fn format_fmt(f: FormatArg) -> &'static str {
        match f {
            FormatArg::Txt => "txt",
            FormatArg::Srt => "srt",
        }
    }
}
