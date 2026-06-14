//! Error types and exit codes for the CLI.
//!
//! All public items return [`AppResult<T>`], which is a `Result<T, AppError>`.
//! The [`AppError`] enum carries the exit code, an English display message,
//! and (when applicable) a structured [`NoSubtitleReason`] that callers can
//! branch on without parsing the error string.

use std::process::{ExitCode, Termination};
use thiserror::Error;

/// BSD sysexits.h constants. See `man 3 sysexits` and
/// <https://man.openbsd.org/sysexits>. Mapped from the legacy 2-7
/// scheme to provide interoperability with downstream POSIX tooling
/// that distinguishes exit codes by category.
pub mod sysexits {
    /// Command line usage error (BSD sysexits.h: `EX_USAGE`).
    pub const EX_USAGE: u8 = 64;
    /// Data format error (BSD sysexits.h: `EX_DATAERR`).
    pub const EX_DATAERR: u8 = 65;
    /// Cannot open input (BSD sysexits.h: `EX_NOINPUT`).
    pub const EX_NOINPUT: u8 = 66;
    /// Service unavailable (BSD sysexits.h: `EX_UNAVAILABLE`).
    pub const EX_UNAVAILABLE: u8 = 69;
    /// Internal software error (BSD sysexits.h: `EX_SOFTWARE`).
    pub const EX_SOFTWARE: u8 = 70;
}

/// Top-level error type returned by every public API in this crate.
///
/// The `Display` impl is the user-facing message written to stderr by
/// [`Termination::report`]. It is intentionally in English so downstream
/// consumers can parse exit codes and pipe error fields.
#[doc(alias = "error")]
#[doc(alias = "Error")]
#[doc(alias = "cli_error")]
#[doc(alias = "exit code")]
#[doc(alias = "sysexits")]
#[doc(alias = "BSD")]
#[doc(alias = "error type")]
#[doc(alias = "thiserror")]
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AppError {
    /// The CLI was invoked with an unsupported combination of flags.
    #[error("invalid usage: {0}")]
    InvalidUsage(String),

    /// A user-supplied input string was rejected by validation.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// stdin was empty or contained only whitespace.
    #[error("stdin is empty")]
    StdinEmpty,

    /// A URL was syntactically valid but not a recognized `YouTube` URL.
    #[error("invalid url: {0}")]
    InvalidUrl(String),

    /// Subtitle lookup succeeded but the video has no subtitle that matches
    /// the request. The inner reason captures *why*.
    #[error("no subtitle: {0}")]
    NoSubtitle(NoSubtitleReason),

    /// Both providers in the chain returned transient errors.
    #[error("providers unavailable")]
    ProviderUnavailable,

    /// Upstream answered HTTP 429. Carries the parsed `Retry-After`
    /// delta-seconds when the provider sent one (EC-021).
    #[error("rate limited by provider (HTTP 429)")]
    RateLimited {
        /// Parsed `Retry-After` value in seconds, when present.
        retry_after_secs: Option<u64>,
    },

    /// An HTTP request exceeded the configured timeout.
    #[error("timeout: {0}")]
    Timeout(String),

    /// Wrapped [`std::io::Error`].
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Wrapped [`reqwest::Error`].
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    /// Wrapped [`url::ParseError`].
    #[error("url parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    /// Wrapped [`serde_json::Error`].
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    /// A cryptographic primitive failed (PBKDF2, AES, etc.).
    #[error("crypto error: {0}")]
    Crypto(String),

    /// Decoded subtitle exceeded the 50 MiB in-memory safety cap.
    #[error("subtitle exceeds limit: {0} bytes")]
    SubtitleTooLarge(usize),

    /// Catch-all for internal invariant violations. Indicates a bug.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Structured reason why a video has no matching subtitle.
///
/// Returned via [`NoSubtitleReason::from_status`] when the upstream provider
/// answers with one of the recognized HTTP status codes, or constructed
/// directly by providers that discover the absence in the response body.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum NoSubtitleReason {
    /// Video is private, members-only, or age-restricted (HTTP 403).
    #[error("video is private or age-restricted (HTTP 403)")]
    PrivateOrAgeRestricted,

    /// Video does not exist (HTTP 404).
    #[error("video not found (HTTP 404)")]
    NotFound,

    /// Video was removed by the author (HTTP 410).
    #[error("video removed by author (HTTP 410)")]
    Gone,

    /// Video is unavailable for legal reasons (HTTP 451).
    #[error("video unavailable for legal reasons (HTTP 451)")]
    UnavailableForLegalReasons,

    /// The video exists but no captions have been published.
    #[error("no captions published for this video")]
    NotPublished,

    /// Captions exist but not in the requested language.
    #[error("requested language is unavailable")]
    LanguageUnavailable,
}

impl NoSubtitleReason {
    /// Map an HTTP status code to a known reason, or `None` if the status
    /// does not correspond to any of the structured cases.
    pub fn from_status(status: u16) -> Option<Self> {
        match status {
            403 => Some(Self::PrivateOrAgeRestricted),
            404 => Some(Self::NotFound),
            410 => Some(Self::Gone),
            451 => Some(Self::UnavailableForLegalReasons),
            _ => None,
        }
    }
}

impl AppError {
    /// Process exit code for this error. See the README exit-code table.
    pub fn exit_code(&self) -> u8 {
        use sysexits::*;
        match self {
            AppError::InvalidUsage(_) | AppError::InvalidInput(_) | AppError::StdinEmpty => {
                EX_USAGE
            }
            AppError::InvalidUrl(_) | AppError::UrlParse(_) => EX_DATAERR,
            AppError::NoSubtitle(_) => EX_NOINPUT,
            AppError::ProviderUnavailable | AppError::RateLimited { .. } => EX_UNAVAILABLE,
            AppError::Timeout(_)
            | AppError::Io(_)
            | AppError::Http(_)
            | AppError::Serde(_)
            | AppError::Crypto(_)
            | AppError::SubtitleTooLarge(_)
            | AppError::Internal(_) => EX_SOFTWARE,
        }
    }

    /// If this is [`AppError::NoSubtitle`], return the inner reason.
    /// Otherwise, return [`NoSubtitleReason::NotPublished`] as a neutral
    /// default so callers can always branch on the reason.
    pub fn reason(&self) -> NoSubtitleReason {
        if let AppError::NoSubtitle(r) = self {
            *r
        } else {
            NoSubtitleReason::NotPublished
        }
    }
}

impl Termination for AppError {
    fn report(self) -> ExitCode {
        tracing::error!(target: "user_error", code = self.exit_code(), "{}", self);
        ExitCode::from(self.exit_code())
    }
}

impl From<AppError> for ExitCode {
    fn from(err: AppError) -> Self {
        ExitCode::from(err.exit_code())
    }
}

/// Convenience alias for `Result<T, AppError>`.
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_subtitle_reason_from_status() {
        assert_eq!(
            NoSubtitleReason::from_status(403),
            Some(NoSubtitleReason::PrivateOrAgeRestricted)
        );
        assert_eq!(
            NoSubtitleReason::from_status(404),
            Some(NoSubtitleReason::NotFound)
        );
        assert_eq!(
            NoSubtitleReason::from_status(410),
            Some(NoSubtitleReason::Gone)
        );
        assert_eq!(
            NoSubtitleReason::from_status(451),
            Some(NoSubtitleReason::UnavailableForLegalReasons)
        );
        assert_eq!(NoSubtitleReason::from_status(500), None);
    }

    #[test]
    fn no_subtitle_exit_code_is_66() {
        let err = AppError::NoSubtitle(NoSubtitleReason::NotPublished);
        assert_eq!(err.exit_code(), 66);
    }

    #[test]
    fn stdin_empty_exit_code_is_64() {
        assert_eq!(AppError::StdinEmpty.exit_code(), 64);
    }

    #[test]
    fn subtitle_too_large_exit_code_is_70() {
        assert_eq!(AppError::SubtitleTooLarge(60_000_000).exit_code(), 70);
    }

    #[test]
    fn timeout_exit_code_is_70() {
        assert_eq!(AppError::Timeout("after 30s".to_string()).exit_code(), 70);
    }

    #[test]
    fn provider_unavailable_exit_code_is_69() {
        assert_eq!(AppError::ProviderUnavailable.exit_code(), 69);
    }

    #[test]
    fn rate_limited_exit_code_is_69() {
        let err = AppError::RateLimited {
            retry_after_secs: Some(60),
        };
        assert_eq!(err.exit_code(), 69);
    }

    #[test]
    fn invalid_url_exit_code_is_65() {
        assert_eq!(AppError::InvalidUrl("bad".to_string()).exit_code(), 65);
    }

    #[test]
    fn internal_error_exit_code_is_70() {
        assert_eq!(AppError::Internal("oops".to_string()).exit_code(), 70);
    }

    #[test]
    fn all_exit_codes_are_in_sysexits_range() {
        let errs = vec![
            AppError::InvalidUsage("x".into()),
            AppError::InvalidInput("x".into()),
            AppError::StdinEmpty,
            AppError::InvalidUrl("x".into()),
            AppError::UrlParse(url::ParseError::EmptyHost),
            AppError::NoSubtitle(NoSubtitleReason::NotPublished),
            AppError::ProviderUnavailable,
            AppError::RateLimited {
                retry_after_secs: None,
            },
            AppError::Timeout("x".into()),
            AppError::Internal("x".into()),
        ];
        for e in errs {
            let code = e.exit_code();
            assert!(
                (64..=78).contains(&code),
                "exit code {code} out of sysexits range 64-78 for {e:?}"
            );
        }
    }

    #[test]
    fn reason_helper_returns_inner_reason() {
        let err = AppError::NoSubtitle(NoSubtitleReason::NotFound);
        assert_eq!(err.reason(), NoSubtitleReason::NotFound);
    }

    #[test]
    fn reason_helper_defaults_to_not_published() {
        let err = AppError::Timeout("x".to_string());
        assert_eq!(err.reason(), NoSubtitleReason::NotPublished);
    }

    #[test]
    fn no_subtitle_reason_messages_are_human_readable() {
        assert!(NoSubtitleReason::PrivateOrAgeRestricted
            .to_string()
            .contains("403"));
        assert!(NoSubtitleReason::UnavailableForLegalReasons
            .to_string()
            .contains("451"));
    }
}
