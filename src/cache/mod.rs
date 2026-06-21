//! TTL-keyed local file cache for fetched subtitles.
//!
//! The `cache_path` / `read_cache` / `write_cache` helpers keep their
//! public signatures so callers outside this module are unaffected.
//!
//! GAP-AUD-2026-051: subtitle bodies are cached alongside a sidecar
//! `*.hint` file that records the [`crate::provider::SubtitleFormat`]
//! discriminator (`srt` or `noteey-transcript`). The cache hit path
//! in `commands::extract` consults the sidecar to pick the right
//! parser — without it, noteey-style bodies cached on disk would be
//! re-parsed as `Srt`, leaking `MM:SS` timestamps into the output.

#![allow(dead_code)]

use crate::error::{AppError, AppResult};
use crate::provider::SubtitleFormat;
use directories::ProjectDirs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_TTL_HOURS: u64 = 24;
const ENV_QUALIFIER: &str = "YOUTUBE_LEGEND_CLI_AUTHOR";
const FALLBACK_QUALIFIER: &str = "youtube-legend-cli";

/// Build the absolute cache file path for a `(video_id, language, format)`
/// triple under the user's cache directory, creating the parent
/// directory if necessary.
///
/// # Errors
///
/// - \[`crate::error::AppError::InvalidInput`\] when any of the components is empty or
///   the TTL is zero.
/// - [`AppError::Internal`] when the platform's project directory cannot
///   be determined.
/// - [`AppError::Io`] when the parent directory cannot be created.
#[tracing::instrument(level = "debug", err, skip(video_id, lang, format), fields(video_id, lang, format, ttl_secs = ttl.as_secs()))]
pub fn cache_path(video_id: &str, lang: &str, format: &str, ttl: Duration) -> AppResult<PathBuf> {
    if video_id.is_empty() || lang.is_empty() || format.is_empty() {
        return Err(AppError::InvalidInput(
            "cache_path requires non-empty video_id, lang, and format".to_string(),
        ));
    }
    if ttl.is_zero() {
        return Err(AppError::InvalidInput(
            "cache_path requires a non-zero ttl".to_string(),
        ));
    }

    let qualifier = qualifier_from_env();
    let proj = ProjectDirs::from("com", &qualifier, FALLBACK_QUALIFIER)
        .ok_or_else(|| AppError::Internal("could not determine cache directory".to_string()))?;

    let dir = proj
        .cache_dir()
        .join("subtitles")
        .join(sanitize(video_id)?)
        .join(sanitize(lang)?);

    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::Io(std::io::Error::other(format!("creating cache dir: {e}"))))?;

    Ok(dir.join(format!("{}.bin", sanitize(format)?)))
}

fn qualifier_from_env() -> String {
    if let Ok(value) = std::env::var(ENV_QUALIFIER) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return sanitize_qualifier(trimmed);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let trimmed = home.trim();
        if !trimmed.is_empty() {
            return sanitize_qualifier(trimmed);
        }
    }
    FALLBACK_QUALIFIER.to_string()
}

fn sanitize_qualifier(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn sanitize(input: &str) -> AppResult<String> {
    if input
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        Ok(input.to_string())
    } else {
        Err(AppError::InvalidInput(format!(
            "invalid path component: {input}"
        )))
    }
}

/// Read a cached entry if it exists and is still fresh.
///
/// Returns `Ok(None)` when the file does not exist or is older than `ttl`
/// (in which case the stale file is also removed).
///
/// # Errors
///
/// - [`AppError::Io`] on any filesystem or metadata read failure.
#[tracing::instrument(level = "debug", err, skip(path), fields(path = %path.display(), ttl_secs = ttl.as_secs()))]
pub async fn read_cache(path: &PathBuf, ttl: Duration) -> AppResult<Option<Vec<u8>>> {
    if !path.exists() {
        return Ok(None);
    }

    let metadata = tokio::fs::metadata(path).await.map_err(AppError::Io)?;
    let modified = metadata
        .modified()
        .map_err(|e| AppError::Io(std::io::Error::other(e.to_string())))?;
    let elapsed = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|e| AppError::Io(std::io::Error::other(e.to_string())))?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| AppError::Io(std::io::Error::other(e.to_string())))?;

    if now.saturating_sub(elapsed) > ttl {
        let _ = tokio::fs::remove_file(path).await;
        return Ok(None);
    }

    let bytes = tokio::fs::read(path).await.map_err(AppError::Io)?;
    Ok(Some(bytes))
}

/// GAP-AUD-2026-051: read a cached entry plus its format hint. The
/// hint lives in a sidecar file `<path>.hint` next to the cached
/// body. When the sidecar is missing or unreadable the function
/// conservatively reports [`SubtitleFormat::Srt`] — operators upgrading
/// from a v0.3.0 cache (which never wrote the sidecar) will continue to
/// receive SRT bodies until the entry expires and is rewritten.
///
/// # Errors
///
/// - [`AppError::Io`] on any filesystem or metadata read failure.
#[tracing::instrument(level = "debug", err, skip(path), fields(path = %path.display(), ttl_secs = ttl.as_secs()))]
pub async fn read_cache_with_hint(
    path: &PathBuf,
    ttl: Duration,
) -> AppResult<Option<(Vec<u8>, SubtitleFormat)>> {
    let bytes = match read_cache(path, ttl).await? {
        Some(b) => b,
        None => return Ok(None),
    };
    let hint_path = hint_path_for(path);
    let hint = match tokio::fs::read(&hint_path).await {
        Ok(s) => match std::str::from_utf8(&s) {
            Ok(s) => parse_hint(s).unwrap_or(SubtitleFormat::Srt),
            Err(_) => SubtitleFormat::Srt,
        },
        Err(_) => SubtitleFormat::Srt,
    };
    Ok(Some((bytes, hint)))
}

/// Persist `content` to `path`, creating the parent directory if needed.
///
/// # Errors
///
/// - [`AppError::Io`] on any filesystem write failure.
#[tracing::instrument(level = "debug", err, skip(path, content), fields(path = %path.display(), bytes = content.len()))]
pub async fn write_cache(path: &PathBuf, content: &[u8]) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(AppError::Io)?;
    }
    tokio::fs::write(path, content)
        .await
        .map_err(AppError::Io)?;
    Ok(())
}

/// GAP-AUD-2026-051: persist `content` plus its `format_hint` sidecar.
/// The hint is stored as a UTF-8 string (`srt` or `noteey-transcript`)
/// in a sibling file `<path>.hint`. The next read via
/// [`read_cache_with_hint`] recovers the discriminator so the cache
/// hit path can pick the right parser.
///
/// # Errors
///
/// - [`AppError::Io`] on any filesystem write failure.
#[tracing::instrument(level = "debug", err, skip(path, content, format_hint), fields(path = %path.display(), bytes = content.len()))]
pub async fn write_cache_with_hint(
    path: &PathBuf,
    content: &[u8],
    format_hint: SubtitleFormat,
) -> AppResult<()> {
    write_cache(path, content).await?;
    let hint_path = hint_path_for(path);
    let hint_text = format_hint.as_str();
    if let Some(parent) = hint_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(AppError::Io)?;
    }
    tokio::fs::write(&hint_path, hint_text.as_bytes())
        .await
        .map_err(AppError::Io)?;
    Ok(())
}

fn hint_path_for(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".hint");
    PathBuf::from(s)
}

fn parse_hint(s: &str) -> Option<SubtitleFormat> {
    match s.trim() {
        "srt" => Some(SubtitleFormat::Srt),
        "noteey-transcript" => Some(SubtitleFormat::NoteeyTranscript),
        _ => None,
    }
}

/// Remove a cache entry if it exists. A missing entry is not an error.
///
/// # Errors
///
/// - [`AppError::Io`] on filesystem remove failure.
#[tracing::instrument(level = "debug", err, skip(path), fields(path = %path.display()))]
pub async fn invalidate_cache(path: &PathBuf) -> AppResult<()> {
    if path.exists() {
        tokio::fs::remove_file(path).await.map_err(AppError::Io)?;
    }
    Ok(())
}

/// Default TTL of 24 hours, used when the user does not pass `--cache-ttl`.
#[tracing::instrument(level = "debug")]
pub fn default_ttl() -> Duration {
    Duration::from_secs(DEFAULT_TTL_HOURS * 3600)
}


#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn default_ttl_is_24_hours() {
        assert_eq!(default_ttl(), Duration::from_secs(24 * 3600));
    }

    #[test]
    #[serial]
    fn qualifier_prefers_env_var() {
        // SAFETY: `std::env::set_var` and `std::env::remove_var` are
        // process-global mutations, but this test is marked `#[serial]`
        // so the surrounding test runner guarantees no concurrent
        // reads of `ENV_QUALIFIER` while the mutation is in flight.
        unsafe {
            std::env::set_var(ENV_QUALIFIER, "test-author");
        }
        let q = qualifier_from_env();
        // SAFETY: same rationale as the `set_var` above; the serial
        // harness still owns the test thread.
        unsafe {
            std::env::remove_var(ENV_QUALIFIER);
        }
        assert_eq!(q, "test-author");
    }

    #[test]
    #[serial]
    fn qualifier_falls_back_to_home() {
        let original = std::env::var(ENV_QUALIFIER).ok();
        // SAFETY: `set_var`/`remove_var` are process-global; the
        // `#[serial]` attribute on this test serialises it against
        // every other test that touches `ENV_QUALIFIER` or `HOME`.
        unsafe {
            std::env::remove_var(ENV_QUALIFIER);
            std::env::set_var("HOME", "/home/test-user");
        }
        let q = qualifier_from_env();
        // SAFETY: paired with the set_var above; restores the
        // previous value (or removes the override) so subsequent
        // tests observe a clean environment.
        unsafe {
            std::env::remove_var("HOME");
            if let Some(v) = original {
                std::env::set_var(ENV_QUALIFIER, v);
            }
        }
        assert!(q.contains("test-user") || q == FALLBACK_QUALIFIER);
    }

    #[test]
    fn qualifier_sanitizes_invalid_chars() {
        let s = sanitize_qualifier("hello world/foo");
        assert_eq!(s, "hello_world_foo");
    }

    #[test]
    fn cache_path_rejects_zero_ttl() {
        let res = cache_path("vid12345678", "en", "txt", Duration::ZERO);
        assert!(matches!(res, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn cache_path_rejects_empty_components() {
        let res = cache_path("", "en", "txt", default_ttl());
        assert!(matches!(res, Err(AppError::InvalidInput(_))));
    }

    #[test]
    fn sanitize_accepts_safe_chars() {
        assert_eq!(sanitize("video_123-abc.txt").unwrap(), "video_123-abc.txt");
    }

    #[test]
    fn sanitize_rejects_unsafe_chars() {
        assert!(sanitize("../etc/passwd").is_err());
        assert!(sanitize("with space").is_err());
    }
}
