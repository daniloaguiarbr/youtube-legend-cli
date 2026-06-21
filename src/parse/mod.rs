//! SRT text extraction and YouTube URL / video-id parsing.

pub mod srv3;
pub mod video_id;

use crate::error::{AppError, AppResult, NoSubtitleReason};
use crate::text::normalize_nfc;
use regex::Regex;
use std::sync::OnceLock;

static TIMESTAMP_RE: OnceLock<Regex> = OnceLock::new();
static INDEX_RE: OnceLock<Regex> = OnceLock::new();
// GAP-AUD-2026-038: noteey.com delivers transcripts as plain text with
// `MM:SS` or `HH:MM:SS` prefixes per line (no SRT framing, no arrow).
// The `OnceLock<Regex>` pattern mirrors `TIMESTAMP_RE` for consistency
// with the rest of this module.
static NOTEEY_TS_RE: OnceLock<Regex> = OnceLock::new();

fn timestamp_re() -> &'static Regex {
    TIMESTAMP_RE.get_or_init(|| {
        Regex::new(r"^\d{2}:\d{2}:\d{2}[,.]?\d{3}\s*-->\s*\d{2}:\d{2}:\d{2}[,.]?\d{3}")
            .expect("static SRT timestamp regex is valid")
    })
}

fn index_re() -> &'static Regex {
    INDEX_RE.get_or_init(|| Regex::new(r"^\d+$").expect("static SRT index regex is valid"))
}

fn noteey_ts_re() -> &'static Regex {
    NOTEEY_TS_RE.get_or_init(|| {
        // Matches `MM:SS` or `HH:MM:SS` followed by an optional
        // `.mmm`/`,mmm` fraction, optionally followed by whitespace.
        // Anchored at start of line so only leading timestamps are
        // stripped. Used for two purposes:
        //   1. Strip the inline timestamp prefix when timestamp and
        //      text share a line (`MM:SS texto`).
        //   2. Detect a standalone-timestamp line (matched without
        //      trailing text), which is paired with the next line.
        Regex::new(r"^\d{2}:\d{2}(?::\d{2})?[.,]?\d*")
            .expect("static noteey timestamp regex is valid")
    })
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

/// GAP-AUD-2026-038: clean a noteey.com transcript body.
///
/// Noteey returns the full transcript as plain text with `MM:SS` (or
/// `HH:MM:SS` for longer videos) leading each line, optionally with a
/// fractional `.mmm`/`,mmm` segment. Unlike SRT, there are no blank
/// lines, no arrow lines, and no numeric index — every line is a cue
/// with a single timestamp prefix.
///
/// This function:
/// 1. Normalises CRLF/CR to LF.
/// 2. Trims each line.
/// 3. Strips the leading timestamp via the internal regex.
/// 4. Drops marker-only lines (e.g. `[Music]`, `(Applause)`, empty
///    after strip) to reduce noise.
/// 5. Joins remaining lines with `\n` (single newline, not blank line
///    like SRT — noteey already separates cues).
/// 6. Normalises Unicode to NFC.
///
/// # Errors
///
/// - [`AppError::NoSubtitle`] when the body is empty or yields zero
///   lines after stripping.
/// - [`AppError::SubtitleTooLarge`] when the body exceeds 50 MiB.
#[tracing::instrument(level = "debug", err, skip(raw), fields(len_bytes = raw.len()))]
pub fn noteey_to_text(raw: &str) -> AppResult<String> {
    if raw.is_empty() {
        return Err(AppError::NoSubtitle(NoSubtitleReason::NotPublished));
    }
    if raw.len() > 50 * 1024 * 1024 {
        return Err(AppError::SubtitleTooLarge(raw.len()));
    }

    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut out: Vec<String> = Vec::new();

    for line in normalized.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // GAP-AUD-2026-047: noteey can render cues as alternating
        // timestamp-only and text-only lines (`00:00\ntexto`). A
        // standalone timestamp line is dropped; the text line below
        // it keeps the cue body. Lines that arrive with timestamp
        // AND text on the same line (`00:00 texto`) get the
        // timestamp stripped via `noteey_ts_re().replace`.
        let matched_ts: Option<String> = noteey_ts_re()
            .find(trimmed)
            .map(|m| m.as_str().to_owned());
        let is_standalone = matched_ts
            .as_deref()
            .map(|ts| ts.len() == trimmed.len())
            .unwrap_or(false);
        if is_standalone {
            // Standalone timestamp with no text. Drop silently —
            // the next text line carries the cue body.
            continue;
        }
        // Text line. Strip any leading timestamp prefix.
        let stripped = noteey_ts_re().replace(trimmed, "").trim().to_owned();
        // GAP-AUD-2026-038 marker-line handling: after stripping
        // the leading timestamp, the line may consist only of a
        // parenthetical `(Applause)` or bracketed `[Music]` marker
        // with no spoken text. Drop those to avoid polluting the
        // transcript with stage directions.
        let is_marker_only = (stripped.starts_with('[') && stripped.ends_with(']'))
            || (stripped.starts_with('(') && stripped.ends_with(')'));
        if stripped.is_empty() || is_marker_only {
            continue;
        }
        out.push(normalize_nfc(&stripped));
    }

    if out.is_empty() {
        return Err(AppError::NoSubtitle(NoSubtitleReason::NotPublished));
    }

    Ok(out.join("\n"))
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

    // GAP-AUD-2026-038 / GAP-AUD-2026-047: noteey_to_text regression
    // tests. The function returns clean plain text — timestamps are
    // stripped and stage-direction markers (`[Music]`, `(Applause)`)
    // are dropped.
    #[test]
    fn noteey_clean_strips_leading_timestamp_prefix() {
        let raw = "00:00 hello\n00:03 world\n00:05 again\n";
        let text = noteey_to_text(raw).unwrap();
        assert_eq!(text, "hello\nworld\nagain");
    }

    #[test]
    fn noteey_clean_handles_milliseconds() {
        let raw = "00:00.123 hello\n00:03.456 world\n";
        let text = noteey_to_text(raw).unwrap();
        assert_eq!(text, "hello\nworld");
    }

    #[test]
    fn noteey_clean_handles_hh_mm_ss_format() {
        let raw = "01:02:03 long video cue\n01:02:08 next cue\n";
        let text = noteey_to_text(raw).unwrap();
        assert_eq!(text, "long video cue\nnext cue");
    }

    #[test]
    fn noteey_clean_skips_empty_lines() {
        let raw = "00:00 first\n\n00:05 second\n\n\n00:10 third\n";
        let text = noteey_to_text(raw).unwrap();
        assert_eq!(text, "first\nsecond\nthird");
    }

    #[test]
    fn noteey_clean_skips_marker_only_lines() {
        let raw = "00:00 [Music]\n00:03 hello\n00:05 (Applause)\n";
        let text = noteey_to_text(raw).unwrap();
        assert_eq!(text, "hello");
    }

    #[test]
    fn noteey_clean_handles_accented_text() {
        let raw = "00:00 Olá mundo\n00:03 ção não\n";
        let text = noteey_to_text(raw).unwrap();
        assert_eq!(text, "Olá mundo\nção não");
    }

    #[test]
    fn noteey_clean_handles_crlf() {
        let raw = "00:00 first\r\n00:03 second\r\n";
        let text = noteey_to_text(raw).unwrap();
        assert_eq!(text, "first\nsecond");
    }

    // GAP-AUD-2026-047: noteey renders cues as alternating timestamp
    // and text lines (live SPA layout). The parser silently drops
    // the standalone timestamp and keeps only the text.
    #[test]
    fn noteey_clean_joins_alternating_timestamp_and_text_lines() {
        let raw = "00:00\nhello\n00:03\nworld\n00:05\nagain\n";
        let text = noteey_to_text(raw).unwrap();
        assert_eq!(text, "hello\nworld\nagain");
    }

    #[test]
    fn noteey_clean_drops_pending_ts_without_followup_text() {
        // A trailing timestamp with no text is dropped silently.
        let raw = "00:00 hello\n00:03 world\n00:05\n";
        let text = noteey_to_text(raw).unwrap();
        assert_eq!(text, "hello\nworld");
    }

    #[test]
    fn noteey_clean_rejects_empty() {
        let err = noteey_to_text("").unwrap_err();
        assert!(matches!(err, AppError::NoSubtitle(_)));
    }

    #[test]
    fn noteey_clean_rejects_only_whitespace() {
        let err = noteey_to_text("   \n\n  \t\n").unwrap_err();
        assert!(matches!(err, AppError::NoSubtitle(_)));
    }

    #[test]
    fn noteey_clean_rejects_only_timestamps() {
        // No cue text after timestamps — purely marker lines.
        let raw = "00:00 [Music]\n00:05 (Applause)\n";
        let err = noteey_to_text(raw).unwrap_err();
        assert!(matches!(err, AppError::NoSubtitle(_)));
    }

    #[test]
    fn noteey_clean_respects_50mb_cap() {
        let big = format!("00:00 {}\n", "a".repeat(51 * 1024 * 1024));
        let err = noteey_to_text(&big).unwrap_err();
        assert!(matches!(err, AppError::SubtitleTooLarge(_)));
    }
}
