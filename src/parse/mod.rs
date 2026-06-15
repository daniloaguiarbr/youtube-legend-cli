//! SRT text extraction and YouTube URL / video-id parsing.

pub mod srv3;
pub mod video_id;

use crate::error::{AppError, AppResult};
use crate::text::normalize_nfc;
use regex::Regex;
use std::sync::OnceLock;

static TIMESTAMP_RE: OnceLock<Regex> = OnceLock::new();
static INDEX_RE: OnceLock<Regex> = OnceLock::new();

fn timestamp_re() -> &'static Regex {
    TIMESTAMP_RE.get_or_init(|| {
        Regex::new(r"^\d{2}:\d{2}:\d{2}[,.]?\d{3}\s*-->\s*\d{2}:\d{2}:\d{2}[,.]?\d{3}")
            .expect("static SRT timestamp regex is valid")
    })
}

fn index_re() -> &'static Regex {
    INDEX_RE.get_or_init(|| Regex::new(r"^\d+$").expect("static SRT index regex is valid"))
}

/// Convert a raw SRT body into plain text, one cue per paragraph.
///
/// Strips numeric indices, `HH:MM:SS,mmm --> HH:MM:SS,mmm` timestamp
/// lines, and joins multi-line cues with a single space. The output is
/// normalised to Unicode NFC.
///
/// # Errors
///
/// - [`AppError::InvalidInput`] when the body is empty or contains no
///   parseable cues.
/// - [`AppError::SubtitleTooLarge`] when the body exceeds 50 MiB.
///
/// # Examples
///
/// ```
/// use youtube_legend_cli::parse::srt_to_text;
///
/// let srt = "1\n00:00:01,000 --> 00:00:02,000\nHello world\n\n\
///            2\n00:00:03,000 --> 00:00:04,000\nSecond cue\n";
/// let text = srt_to_text(srt).unwrap();
/// assert_eq!(text, "Hello world\n\nSecond cue");
/// ```
#[tracing::instrument(level = "debug", err, skip(srt), fields(len_bytes = srt.len()))]
pub fn srt_to_text(srt: &str) -> AppResult<String> {
    if srt.is_empty() {
        return Err(AppError::InvalidInput("empty srt body".to_string()));
    }
    if srt.len() > 50 * 1024 * 1024 {
        return Err(AppError::SubtitleTooLarge(srt.len()));
    }

    let normalized = srt.replace("\r\n", "\n").replace('\r', "\n");

    let mut cues: Vec<String> = Vec::new();
    let mut current_lines: Vec<String> = Vec::new();

    for line in normalized.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !current_lines.is_empty() {
                let text = join_cue_lines(&current_lines);
                if !text.is_empty() {
                    cues.push(normalize_nfc(&text));
                }
                current_lines.clear();
            }
            continue;
        }
        if index_re().is_match(trimmed) {
            continue;
        }
        if timestamp_re().is_match(trimmed) {
            continue;
        }
        current_lines.push(trimmed.to_string());
    }

    if !current_lines.is_empty() {
        let text = join_cue_lines(&current_lines);
        if !text.is_empty() {
            cues.push(normalize_nfc(&text));
        }
    }

    if cues.is_empty() {
        return Err(AppError::InvalidInput("srt has no valid cues".to_string()));
    }

    Ok(cues.join("\n\n"))
}

fn join_cue_lines(lines: &[String]) -> String {
    lines.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_srt() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\nHello world\n\n2\n00:00:03,000 --> 00:00:04,000\nSecond cue\n";
        let text = srt_to_text(srt).unwrap();
        assert_eq!(text, "Hello world\n\nSecond cue");
    }

    #[test]
    fn handles_crlf() {
        let srt = "1\r\n00:00:01,000 --> 00:00:02,000\r\nLine one\r\nLine two\r\n";
        let text = srt_to_text(srt).unwrap();
        assert_eq!(text, "Line one Line two");
    }

    #[test]
    fn handles_multiline_cue() {
        let srt = "1\n00:00:01,000 --> 00:00:05,000\nLine one\nLine two\nLine three\n";
        let text = srt_to_text(srt).unwrap();
        assert_eq!(text, "Line one Line two Line three");
    }

    #[test]
    fn rejects_empty() {
        let err = srt_to_text("").unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn rejects_over_50mb() {
        let big = "a".repeat(51 * 1024 * 1024);
        let err = srt_to_text(&big).unwrap_err();
        assert!(matches!(err, AppError::SubtitleTooLarge(_)));
    }

    #[test]
    fn handles_accented_text() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\nOlá mundo com acentuação\n";
        let text = srt_to_text(srt).unwrap();
        assert_eq!(text, "Olá mundo com acentuação");
    }
}
