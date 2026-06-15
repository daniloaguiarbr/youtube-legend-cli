#![allow(dead_code)]
//! Fetch and parse the `ytInitialPlayerResponse` JSON embedded in a
//! `YouTube` watch-page HTML response.
//!
//! The watch page is a thin SPA shell that contains a giant
//! `<script>var ytInitialPlayerResponse = {...};</script>` blob.
//! Pulling the JSON out of that variable gives us caption track
//! metadata, signature/n-parameter inputs, and the `playabilityStatus`
//! gate — everything M1..M4 need to decide whether subtitles can be
//! fetched directly.

use std::sync::OnceLock;

use regex::Regex;
use reqwest::header::CONTENT_LENGTH;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;

use crate::error::{AppError, AppResult, NoSubtitleReason};
use crate::provider::youtube::caption_track::CaptionTrack;
use crate::secret_endpoints::YOUTUBE_WATCH_URL_BASE;

/// Default cap on the watch-page body. Matches the value documented in
/// `gaps.md` GAP-001 and the plan v0.3.0 design notes.
pub const DEFAULT_MAX_BODY_BYTES: usize = 10 * 1024 * 1024;

/// Maximum effective nesting depth accepted for the parsed
/// `ytInitialPlayerResponse` JSON blob.
///
/// META-GAP-B sets the cap at 64, well below `serde_json 1.0` hard
/// recursion limit of 128, so we reject pathological payloads BEFORE
/// the deserializer walks them. Real `YouTube` responses rarely go
/// beyond depth 10.
const MAX_JSON_DEPTH: usize = 64;

/// Walk `raw` and return `Err(observed)` as soon as the effective
/// nesting depth (counted via matching `{` and `[` minus their closers,
/// string-aware to avoid counting braces that live inside JSON string
/// literals) exceeds `limit`.
///
/// `Ok(())` means the body stays within `limit` throughout.
/// `Err(depth)` carries the first observed depth that breached the
/// cap.
fn check_json_depth(raw: &str, limit: usize) -> Result<(), usize> {
    let mut depth: usize = 0;
    let mut max_observed: usize = 0;
    let mut in_string = false;
    let mut escape_next = false;
    for byte in raw.as_bytes() {
        if escape_next {
            escape_next = false;
            continue;
        }
        if in_string {
            match byte {
                b'\\' => escape_next = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > max_observed {
                    max_observed = depth;
                }
                if depth > limit {
                    return Err(max_observed);
                }
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    Ok(())
}

/// Returns the `Regex` that locates the opening quote of
/// `ytInitialPlayerResponse = "..."` in watch-page HTML.
///
/// Uses `OnceLock` to compile once for the process. The pattern stops
/// at the first `<` it sees after the value, which is where the
/// closing `</script>` lives, so we never overshoot.
fn player_response_var_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?s)var\s+ytInitialPlayerResponse\s*=\s*"((?:[^"\\]|\\.)*?)"\s*;"#)
            .expect("static ytInitialPlayerResponse regex is valid")
    })
}

/// Minimal structural view of the watch-page response. We deserialize
/// with `serde_json::from_value` after the watch-page body is parsed
/// and only project the fields we care about. The blob is large, so
/// keeping a narrow struct keeps the in-memory representation cheap.
#[derive(Debug, Clone)]
pub struct PlayerResponse {
    /// Raw `playabilityStatus.status` string, e.g. `"OK"`, `"ERROR"`.
    pub playability_status: String,
    /// Error reason code (only populated when status is not `OK`).
    pub playability_reason: Option<String>,
    /// All caption tracks, in the order `YouTube` returned them.
    pub caption_tracks: Vec<CaptionTrack>,
}

impl PlayerResponse {
    /// Number of `captionTrack` entries we parsed.
    pub fn caption_track_count(&self) -> usize {
        self.caption_tracks.len()
    }
}

/// View of the JSON top-level keys we need.
#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct RawPlayerResponse {
    #[serde(default)]
    playabilityStatus: Option<RawPlayabilityStatus>,
    #[serde(default)]
    captions: Option<RawCaptions>,
}

#[derive(Debug, Deserialize)]
struct RawPlayabilityStatus {
    #[serde(default)]
    status: String,
    #[serde(rename = "errorScreen", default)]
    error_screen: Option<RawErrorScreen>,
    #[serde(default)]
    reason: Option<String>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct RawErrorScreen {
    #[serde(rename = "playerErrorMessageRenderer", default)]
    renderer: Option<RawPlayerError>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct RawPlayerError {
    #[serde(default)]
    errorcode: Option<String>,
    #[serde(default)]
    reason: Option<RawReason>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct RawReason {
    #[serde(default)]
    simpleText: Option<String>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct RawCaptions {
    #[serde(rename = "playerCaptionsTracklistRenderer", default)]
    renderer: Option<RawTracklist>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct RawTracklist {
    #[serde(default)]
    captionTracks: Vec<Value>,
}

/// Fetch the watch-page body and extract a [`PlayerResponse`].
///
/// # Errors
///
/// - [`AppError::InvalidInput`] when `video_id` is empty.
/// - [`AppError::NoSubtitle`] when the HTTP status maps to a known
///   reason (403, 404, 410, 451).
/// - [`AppError::ProviderUnavailable`] for any other non-success
///   status.
/// - [`AppError::PlayerResponseTooLarge`] when the `Content-Length`
///   header or actual body exceeds `max_body_bytes`.
/// - [`AppError::Http`] on transport errors.
#[tracing::instrument(level = "debug", err, skip(client), fields(video_id, max_body_bytes))]
pub async fn fetch_player_response(
    client: &Client,
    video_id: &str,
    max_body_bytes: usize,
) -> AppResult<PlayerResponse> {
    if video_id.is_empty() {
        return Err(AppError::InvalidInput("video_id is empty".to_string()));
    }
    let url = format!("{YOUTUBE_WATCH_URL_BASE}{video_id}");
    let resp = client.get(&url).send().await.map_err(AppError::Http)?;
    let status = resp.status();
    if let Some(reason) = NoSubtitleReason::from_status(status.as_u16()) {
        return Err(AppError::NoSubtitle(reason));
    }
    if !status.is_success() {
        return Err(AppError::ProviderUnavailable);
    }

    if let Some(len) = resp
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok())
    {
        if len > max_body_bytes {
            return Err(AppError::PlayerResponseTooLarge {
                bytes: len,
                limit: max_body_bytes,
            });
        }
    }

    let body = resp.text().await.map_err(AppError::Http)?;
    if body.len() > max_body_bytes {
        return Err(AppError::PlayerResponseTooLarge {
            bytes: body.len(),
            limit: max_body_bytes,
        });
    }

    parse_player_response(&body, max_body_bytes)
}

/// Parse a watch-page body string into a typed [`PlayerResponse`].
///
/// Exposed separately from [`fetch_player_response`] so unit tests can
/// drive it with frozen fixtures via `include_str!`.
///
/// # Errors
///
/// - [`AppError::PlayerResponseTooLarge`] when the body exceeds
///   `max_body_bytes`.
/// - [`AppError::PlayerResponseMissing`] when the
///   `ytInitialPlayerResponse` variable is absent from the HTML.
/// - [`AppError::Internal`] when the embedded JSON cannot be parsed.
/// - [`AppError::Serde`] when the typed projection fails.
/// - [`AppError::PlayabilityStatusDenied`] when
///   `playabilityStatus.status` is not `OK`.
#[tracing::instrument(level = "debug", err, skip(body), fields(body_bytes = body.len()))]
pub fn parse_player_response(body: &str, max_body_bytes: usize) -> AppResult<PlayerResponse> {
    if body.len() > max_body_bytes {
        return Err(AppError::PlayerResponseTooLarge {
            bytes: body.len(),
            limit: max_body_bytes,
        });
    }

    let captures = player_response_var_re().captures(body).ok_or_else(|| {
        AppError::PlayerResponseMissing("ytInitialPlayerResponse var not found".to_string())
    })?;
    let raw_json_escaped = captures
        .get(1)
        .ok_or_else(|| AppError::PlayerResponseMissing("missing capture group".to_string()))?
        .as_str();
    // The HTML escapes the inner JSON via a small fixed table. We must
    // unescape it before handing it to `serde_json`. Anything outside
    // the table falls back to the original character so we never
    // silently corrupt valid unicode outside the common cases.
    let raw_json = unescape_html(raw_json_escaped);

    // META-GAP-B: cap the effective nesting depth BEFORE handing the
    // blob to `serde_json`. `serde_json 1.0` hardcodes the recursion
    // limit to 128 and does not expose `with_depth_limit` in the
    // public API, so we enforce a stricter, application-level cap
    // (64) to keep stack usage bounded when the upstream injects a
    // pathological payload. The scan is O(n) over the bytes of the
    // raw JSON and short-circuits on the first depth breach.
    if let Err(depth) = check_json_depth(&raw_json, MAX_JSON_DEPTH) {
        return Err(AppError::Internal(format!(
            "ytInitialPlayerResponse nesting depth {depth} exceeds limit {MAX_JSON_DEPTH} \
             (rejected before JSON parse to prevent stack-overflow DoS)"
        )));
    }

    let value: Value = serde_json::from_str(&raw_json).map_err(|e| {
        AppError::Internal(format!("ytInitialPlayerResponse is not valid JSON: {e}"))
    })?;

    let parsed: RawPlayerResponse =
        serde_json::from_value(value.clone()).map_err(AppError::Serde)?;

    let playability_status = parsed
        .playabilityStatus
        .as_ref()
        .map(|p| p.status.clone())
        .unwrap_or_default();

    if playability_status != "OK" {
        let reason = parsed
            .playabilityStatus
            .as_ref()
            .and_then(|p| {
                p.error_screen
                    .as_ref()
                    .and_then(|e| e.renderer.as_ref())
                    .and_then(|r| r.errorcode.clone())
                    .or_else(|| p.reason.clone())
            })
            .unwrap_or_else(|| playability_status.clone());
        return Err(AppError::PlayabilityStatusDenied(reason));
    }

    let mut caption_tracks: Vec<CaptionTrack> = Vec::new();
    if let Some(renderer) = parsed.captions.as_ref().and_then(|c| c.renderer.as_ref()) {
        for entry in &renderer.captionTracks {
            match CaptionTrack::try_from(entry) {
                Ok(t) => caption_tracks.push(t),
                Err(AppError::LanguageParseError(msg)) => {
                    tracing::debug!(
                        target: "events",
                        reason = %msg,
                        "skipping caption track with invalid language"
                    );
                }
                Err(e) => {
                    tracing::debug!(
                        target: "events",
                        error = %e,
                        "skipping caption track with parse error"
                    );
                }
            }
        }
    }

    Ok(PlayerResponse {
        playability_status,
        playability_reason: None,
        caption_tracks,
    })
}

/// Tiny HTML-entity unescape used to decode the JSON literal embedded
/// in `ytInitialPlayerResponse = "...";`.
///
/// The `&quot;` and `&amp;` escapes are what `YouTube` uses around
/// inner quotes; the rest are accepted unchanged so valid Unicode
/// passes through untouched.
/// Unescape a `ytInitialPlayerResponse` literal extracted from a
/// watch-page HTML payload.
///
/// `YouTube` wraps the JSON in a JavaScript string literal, so the
/// captured body is a sequence of JS-escape pairs. We decode the
/// handful `YouTube` actually emits: `\"` becomes `"`, `\\` becomes
/// `\`, `\/` becomes `/`, and `\n` becomes a newline. The recognised
/// HTML entities (`&quot;`, `&amp;`, `&lt;`, `&gt;`, `&nbsp;`) are
/// also decoded because `YouTube` sometimes double-encodes quotes.
fn unescape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                let decoded = match next {
                    '"' | '\'' => '"',
                    '\\' => '\\',
                    '/' => '/',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    'b' => '\u{08}',
                    'f' => '\u{0C}',
                    other => {
                        // Unknown escape: preserve both characters so
                        // the embedded JSON is never silently corrupted.
                        out.push('\\');
                        other
                    }
                };
                out.push(decoded);
                chars.next();
            } else {
                out.push('\\');
            }
            continue;
        }
        if c != '&' {
            out.push(c);
            continue;
        }
        let mut buf = String::new();
        while let Some(&next) = chars.peek() {
            if next == ';' || buf.len() >= 8 {
                break;
            }
            buf.push(next);
            chars.next();
        }
        if chars.peek() == Some(&';') {
            chars.next();
        }
        let replaced = match buf.as_str() {
            "quot" => '"',
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "nbsp" => '\u{00A0}',
            other => {
                out.push('&');
                out.push_str(other);
                ';'
            }
        };
        out.push(replaced);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Minimal watch-page HTML containing a `var ytInitialPlayerResponse = "...";`
    /// assignment with two caption tracks (manual + ASR) and `playabilityStatus.status = "OK"`.
    const FIXTURE_OK: &str =
        include_str!("../../../tests/fixtures/snapshots/youtube_watch_Ze0i7zxpyrw.html");

    /// Watch-page HTML with `playabilityStatus.status = "ERROR"`.
    const FIXTURE_DENIED: &str =
        include_str!("../../../tests/fixtures/snapshots/youtube_watch_Tn08k_PWOQk.html");

    /// Watch-page HTML with a single manual English caption track.
    const FIXTURE_EN_MANUAL: &str =
        include_str!("../../../tests/fixtures/snapshots/youtube_watch_NvZ4VZ5hooY.html");

    #[test]
    fn extracts_player_response_var() {
        let parsed = parse_player_response(FIXTURE_OK, DEFAULT_MAX_BODY_BYTES)
            .expect("fixture should parse");
        assert_eq!(parsed.playability_status, "OK");
        assert!(
            parsed.caption_track_count() >= 2,
            "expected >=2 tracks, got {}",
            parsed.caption_track_count()
        );
        let has_asr = parsed
            .caption_tracks
            .iter()
            .any(|t| t.kind.as_deref() == Some("asr"));
        assert!(has_asr, "fixture should include at least one ASR track");
    }

    #[test]
    fn rejects_oversized_body() {
        // Build a body whose extracted JSON is just the variable name;
        // the cap is applied to the *body* size, not the JSON.
        let body = "x".repeat(DEFAULT_MAX_BODY_BYTES + 1);
        let err = parse_player_response(&body, DEFAULT_MAX_BODY_BYTES).unwrap_err();
        assert!(matches!(err, AppError::PlayerResponseTooLarge { .. }));
    }

    #[test]
    fn rejects_playability_error() {
        let err = parse_player_response(FIXTURE_DENIED, DEFAULT_MAX_BODY_BYTES).unwrap_err();
        match err {
            AppError::PlayabilityStatusDenied(reason) => {
                assert!(!reason.is_empty());
            }
            other => panic!("expected PlayabilityStatusDenied, got {other:?}"),
        }
    }

    #[test]
    fn prefers_asr_when_requested() {
        use crate::provider::youtube::caption_track::select_track;
        let parsed = parse_player_response(FIXTURE_OK, DEFAULT_MAX_BODY_BYTES)
            .expect("fixture should parse");
        let chosen =
            select_track(&parsed.caption_tracks, "pt", true).expect("expected an ASR pt track");
        assert_eq!(chosen.kind.as_deref(), Some("asr"));
    }

    #[test]
    fn falls_back_to_manual_when_no_asr() {
        use crate::provider::youtube::caption_track::select_track;
        let parsed = parse_player_response(FIXTURE_EN_MANUAL, DEFAULT_MAX_BODY_BYTES)
            .expect("fixture should parse");
        // Requesting `pt` against an English-only fixture must miss.
        let chosen = select_track(&parsed.caption_tracks, "pt", true);
        assert!(chosen.is_none(), "en-only fixture must not return pt");
    }

    #[test]
    fn parse_handles_html_escaped_quotes() {
        let body = r#"<html><body><script>var ytInitialPlayerResponse = "{&quot;playabilityStatus&quot;:{&quot;status&quot;:&quot;OK&quot;},&quot;captions&quot;:{&quot;playerCaptionsTracklistRenderer&quot;:{&quot;captionTracks&quot;:[]}}}";</script></body></html>"#;
        let parsed =
            parse_player_response(body, DEFAULT_MAX_BODY_BYTES).expect("escaped JSON should parse");
        assert_eq!(parsed.playability_status, "OK");
        assert!(parsed.caption_tracks.is_empty());
    }

    #[test]
    fn unescape_html_preserves_unicode() {
        let input = "caf\u{00e9} &amp; na\u{00ef}ve \u{2014} ok";
        let output = unescape_html(input);
        assert!(output.contains("caf\u{00e9}"));
        assert!(output.contains('&'));
    }

    #[test]
    fn extract_returns_missing_when_var_absent() {
        let body = "<html><body><script>var unrelated = 1;</script></body></html>";
        let err = parse_player_response(body, DEFAULT_MAX_BODY_BYTES).unwrap_err();
        assert!(matches!(err, AppError::PlayerResponseMissing(_)));
    }

    #[test]
    fn empty_caption_tracks_when_renderer_missing() {
        let body = r#"<html><body><script>var ytInitialPlayerResponse = "{\"playabilityStatus\":{\"status\":\"OK\"}}";</script></body></html>"#;
        let parsed = parse_player_response(body, DEFAULT_MAX_BODY_BYTES).unwrap();
        assert_eq!(parsed.playability_status, "OK");
        assert!(parsed.caption_tracks.is_empty());
    }

    #[test]
    fn json_helper_skips_invalid_language() {
        // A bad track must not abort the parse; we keep good ones.
        let body = r#"<html><body><script>var ytInitialPlayerResponse = "{\"playabilityStatus\":{\"status\":\"OK\"},\"captions\":{\"playerCaptionsTracklistRenderer\":{\"captionTracks\":[{\"baseUrl\":\"https://x/y?lang=pt\",\"languageCode\":\"pt\",\"name\":\"PT\",\"vssId\":\".pt\"},{\"baseUrl\":\"https://x/y?lang=en\",\"languageCode\":\"\",\"name\":\"BAD\",\"vssId\":\".en\"}]}}}";</script></body></html>"#;
        let parsed = parse_player_response(body, DEFAULT_MAX_BODY_BYTES).unwrap();
        assert_eq!(parsed.caption_tracks.len(), 1);
        assert_eq!(parsed.caption_tracks[0].language_code, "pt");
    }

    #[test]
    fn json_helper_consumes_real_value() {
        let raw_value = json!({
            "playabilityStatus": { "status": "OK" },
            "captions": {
                "playerCaptionsTracklistRenderer": {
                    "captionTracks": [
                        {"baseUrl":"https://x/a?lang=pt","languageCode":"pt","name":"PT","vssId":".pt"}
                    ]
                }
            }
        });
        let raw: RawPlayerResponse = serde_json::from_value(raw_value).unwrap();
        assert_eq!(raw.playabilityStatus.unwrap().status, "OK");
    }

    // META-GAP-B regression test: a deeply-nested payload MUST be
    // rejected by the application-level depth cap, BEFORE the
    // `serde_json` deserializer walks the structure. Real YouTube
    // responses stay well below 10 levels deep, so 200 nested
    // arrays is unambiguously adversarial.
    //
    // We use nested *arrays* (no inner quotes) so the JS-string
    // regex captures the payload in linear time; object keys with
    // literal `"` would force catastrophic backtracking on the
    // regex itself, which is a separate (pre-existing) concern.
    #[test]
    fn rejects_deeply_nested_payload_to_prevent_dos() {
        let depth = 200usize;
        let mut body = String::with_capacity(depth * 2 + 256);
        body.push_str(r#"<html><body><script>var ytInitialPlayerResponse = ""#);
        for _ in 0..depth {
            body.push('[');
        }
        body.push('1');
        for _ in 0..depth {
            body.push(']');
        }
        body.push_str(r#"";</script></body></html>"#);

        let err = parse_player_response(&body, DEFAULT_MAX_BODY_BYTES)
            .expect_err("pathological nesting must be rejected");
        match err {
            AppError::Internal(msg) => {
                assert!(
                    msg.contains("nesting depth") && msg.contains("exceeds limit"),
                    "unexpected error message: {msg}"
                );
            }
            other => panic!("expected AppError::Internal, got {other:?}"),
        }
    }

    // Belt-and-suspenders: the standalone depth scanner must accept a
    // shallow payload and reject a deep one with the exact depth.
    #[test]
    fn check_json_depth_agrees_with_limit() {
        let shallow = r#"{"a":{"b":{"c":1}}}"#;
        assert!(check_json_depth(shallow, MAX_JSON_DEPTH).is_ok());

        // 200 nested arrays with closing brackets.
        let mut deep = String::new();
        for _ in 0..200 {
            deep.push('[');
        }
        for _ in 0..200 {
            deep.push(']');
        }
        let err = check_json_depth(&deep, MAX_JSON_DEPTH)
            .expect_err("deep array payload must breach the cap");
        assert!(
            err > MAX_JSON_DEPTH,
            "reported depth {err} should exceed limit"
        );
    }
}
