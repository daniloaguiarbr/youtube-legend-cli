//! Srv3 (XML) and JSON3 subtitle formats -> `SubRip` (SRT) conversion.
//!
//! `YouTube` serves timed-text in one of three formats depending on
//! the `fmt` query parameter:
//!
//! - `fmt=json3` — nested JSON, the most common. Each event carries
//!   `tStartMs` (ms) and `dDurationMs` (ms), with the rendered text
//!   spread across one or more `segs[].utf8` chunks.
//! - `fmt=srv3` — flat XML where each `<text start="..." dur="...">`
//!   element holds the cue body. Newlines inside a cue are literal
//!   (the source carries a raw `\n`).
//! - `fmt=srv1` — binary protobuf. **Not implemented in M2**; we
//!   return `AppError::Internal` so the caller can fall back to
//!   `json3` or `srv3` before giving up.
//!
//! Both Srv3 and JSON3 inputs are converted to the `SubRip` wire
//! format the rest of this crate already understands:
//!
//! ```text
//! 1
//! 00:00:01,000 --> 00:00:03,500
//! First cue
//!
//! 2
//! 00:00:04,000 --> 00:00:06,000
//! Second cue
//! ```
//!
//! Index numbers are 1-based. Timestamps use the
//! `HH:MM:SS,mmm` convention with a literal comma between seconds
//! and milliseconds (`SubRip`, not `WebVTT`). Cue text is
//! line-split on raw newlines and the literal ` -->` sequence
//! (which the SRT parser would otherwise mistake for a new
//! timestamp) is escaped to a zero-width-space-prefixed form so
//! `srt_to_text` round-trips cleanly.

use std::sync::OnceLock;

use regex::Regex;

use crate::error::{AppError, AppResult};

/// Cap on a single timed-text body. Anything above this is rejected
/// before any parse attempt. Matches the 5 MiB figure in the v0.3.0
/// plan (gaps.md GAP-001 / T2).
pub const MAX_BODY_BYTES: usize = 5 * 1024 * 1024;

/// Regex that locates every `<text start="…" dur="…">…</text>` cue
/// in an Srv3 XML body. Compiled once per process via [`OnceLock`].
///
/// Notes on the pattern:
/// - `(?s)` enables dot-matches-newline so `.*?` can span a cue
///   body that contains literal `\n` characters.
/// - `start` and `dur` are required and parsed as floats; the
///   `YouTube` timed-text service emits seconds with a decimal
///   point (e.g. `start="2.5"`).
/// - The body of the cue is captured into group 1.
fn srv3_text_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?s)<text\s+start="([0-9.]+)"\s+dur="([0-9.]+)"[^>]*>(.*?)</text>"#)
            .expect("static srv3 text regex is valid")
    })
}

/// Convert a `fmt=srv3` XML body into a `SubRip` (SRT) string.
///
/// # Errors
///
/// - [`AppError::InvalidInput`] when the body is empty or contains
///   no `<text>` cues.
/// - [`AppError::SubtitleTooLarge`] when the body exceeds
///   [`MAX_BODY_BYTES`].
#[tracing::instrument(level = "debug", err, skip(xml), fields(len_bytes = xml.len()))]
pub fn srv3_to_srt(xml: &str) -> AppResult<String> {
    let body = xml.trim();
    if body.is_empty() {
        return Err(AppError::InvalidInput("empty srv3 body".to_string()));
    }
    if body.len() > MAX_BODY_BYTES {
        return Err(AppError::SubtitleTooLarge(body.len()));
    }

    let mut out = String::new();
    let mut index: usize = 0;
    for cap in srv3_text_re().captures_iter(body) {
        let start = cap.get(1).map_or("", |m| m.as_str());
        let dur = cap.get(2).map_or("", |m| m.as_str());
        let text = cap.get(3).map_or("", |m| m.as_str());

        let start_secs: f64 = start
            .parse()
            .map_err(|_| AppError::InvalidInput(format!("srv3 start={start:?} not a float")))?;
        let dur_secs: f64 = dur
            .parse()
            .map_err(|_| AppError::InvalidInput(format!("srv3 dur={dur:?} not a float")))?;
        let end_secs = start_secs + dur_secs;

        index += 1;
        out.push_str(&format!("{index}\n"));
        out.push_str(&format!(
            "{} --> {}\n",
            format_timestamp(start_secs),
            format_timestamp(end_secs)
        ));
        out.push_str(&sanitize_cue_text(text));
        out.push('\n');
    }

    if index == 0 {
        return Err(AppError::InvalidInput(
            "srv3 body has no <text> cues".to_string(),
        ));
    }

    Ok(out)
}

/// Convert a `fmt=json3` JSON body into a `SubRip` (SRT) string.
///
/// The JSON is parsed with `serde_json::from_str` using a narrow
/// `serde_json::Value` projection — only the `events[*].tStartMs`,
/// `dDurationMs`, and `segs[*].utf8` fields are read; everything
/// else (timingRef, voice, formatting markers, etc.) is ignored.
///
/// # Errors
///
/// - [`AppError::InvalidInput`] when the body is empty, not JSON,
///   or contains no usable events.
/// - [`AppError::SubtitleTooLarge`] when the body exceeds
///   [`MAX_BODY_BYTES`].
/// - [`AppError::Serde`] when the JSON is structurally invalid.
#[tracing::instrument(level = "debug", err, skip(json), fields(len_bytes = json.len()))]
pub fn json3_to_srt(json: &str) -> AppResult<String> {
    let body = json.trim();
    if body.is_empty() {
        return Err(AppError::InvalidInput("empty json3 body".to_string()));
    }
    if body.len() > MAX_BODY_BYTES {
        return Err(AppError::SubtitleTooLarge(body.len()));
    }

    let value: serde_json::Value = serde_json::from_str(body).map_err(AppError::Serde)?;
    let events = value
        .get("events")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| AppError::InvalidInput("json3 body has no events[] array".to_string()))?;

    let mut out = String::new();
    let mut index: usize = 0;
    for event in events {
        let t_start_ms = event.get("tStartMs").and_then(serde_json::Value::as_i64);
        let d_dur_ms = event
            .get("dDurationMs")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        let (Some(start_ms), dur_ms) = (t_start_ms, d_dur_ms) else {
            continue;
        };

        let segs = match event.get("segs").and_then(serde_json::Value::as_array) {
            Some(segs) => segs,
            None => continue,
        };

        let mut text = String::new();
        let mut first = true;
        for seg in segs {
            if let Some(utf8) = seg.get("utf8").and_then(serde_json::Value::as_str) {
                if !first && !utf8.is_empty() {
                    text.push('\n');
                }
                text.push_str(utf8);
                first = false;
            }
        }
        if text.is_empty() {
            continue;
        }

        let start_secs = start_ms as f64 / 1000.0;
        let end_secs = (start_ms + dur_ms) as f64 / 1000.0;

        index += 1;
        out.push_str(&format!("{index}\n"));
        out.push_str(&format!(
            "{} --> {}\n",
            format_timestamp(start_secs),
            format_timestamp(end_secs)
        ));
        out.push_str(&sanitize_cue_text(&text));
        out.push('\n');
    }

    if index == 0 {
        return Err(AppError::InvalidInput(
            "json3 body has no usable events".to_string(),
        ));
    }

    Ok(out)
}

/// Format a `seconds` value as `HH:MM:SS,mmm` per the `SubRip`
/// convention. Negative values and `NaN` clamp to `00:00:00,000`
/// so the function never panics on hostile input.
fn format_timestamp(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "00:00:00,000".to_string();
    }
    let total_ms = (seconds * 1000.0).round() as u64;
    let hours = total_ms / 3_600_000;
    let minutes = (total_ms / 60_000) % 60;
    let secs = (total_ms / 1000) % 60;
    let ms = total_ms % 1000;
    format!("{hours:02}:{minutes:02}:{secs:02},{ms:03}")
}

/// Replace the literal ` -->` sequence (which the SRT parser
/// interprets as a new cue timestamp) with a zero-width-space-
/// prefixed form. The character `\u{200B}` is invisible in every
/// SRT renderer we know of, so the cue reads identically while
/// staying parse-safe.
fn sanitize_cue_text(text: &str) -> String {
    let stripped = text.replace("\r\n", "\n").replace('\r', "\n");
    stripped.replace(" -->", "\u{200B}-->")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_srv3() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<transcript>
  <text start="0.0" dur="2.5">Hello world</text>
  <text start="2.5" dur="3.0">Second cue</text>
</transcript>"#;
        let srt = srv3_to_srt(xml).expect("srv3 parses");
        assert!(srt.contains("1\n00:00:00,000 --> 00:00:02,500\nHello world\n"));
        assert!(srt.contains("2\n00:00:02,500 --> 00:00:05,500\nSecond cue\n"));
    }

    #[test]
    fn parses_multiline_cue() {
        let xml = r#"<?xml version="1.0"?>
<transcript>
  <text start="0.0" dur="4.0">Line 1
Line 2
Line 3</text>
</transcript>"#;
        let srt = srv3_to_srt(xml).expect("multiline parses");
        assert!(srt.contains("Line 1\nLine 2\nLine 3"));
        assert!(srt.contains("00:00:00,000 --> 00:00:04,000"));
    }

    #[test]
    fn parses_unicode_cue() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<transcript>
  <text start="0.0" dur="2.0">café 日本</text>
</transcript>"#;
        let srt = srv3_to_srt(xml).expect("unicode parses");
        assert!(srt.contains("café 日本"));
    }

    #[test]
    fn rejects_empty_body() {
        let err = srv3_to_srt("").unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn parses_json3_format() {
        let json = r#"{
            "events": [
                {"tStartMs": 0, "dDurationMs": 2500, "segs": [{"utf8": "Hello world"}]},
                {"tStartMs": 2500, "dDurationMs": 3000, "segs": [{"utf8": "Line 1"}, {"utf8": "Line 2"}]}
            ]
        }"#;
        let srt = json3_to_srt(json).expect("json3 parses");
        assert!(srt.contains("1\n00:00:00,000 --> 00:00:02,500\nHello world\n"));
        assert!(srt.contains("2\n00:00:02,500 --> 00:00:05,500\nLine 1\nLine 2\n"));
    }
}
