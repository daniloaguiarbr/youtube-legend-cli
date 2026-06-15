//! `CaptionTrack` model extracted from the `YouTube` `playerResponse` JSON.
//!
//! The `YouTube` embed watches a single language at a time. Each
//! `captionTrack` node in `playerCaptionsTracklistRenderer.captionTracks`
//! has a `baseUrl`, an ISO 639-1 `languageCode`, a human-readable
//! `name`, an internal `vssId`, and an optional `kind` of `"asr"` for
//! auto-generated captions. We convert that JSON into a typed Rust
//! struct so the rest of the provider never touches raw `Value`.

use serde::Deserialize;
use url::Url;

use crate::error::AppError;

/// A single caption track offered by a `YouTube` video.
///
/// The track selection policy in
/// `crate::provider::Provider::fetch_subtitle` is
/// language-first, then ASR/manual preference, then
/// `trackOrder`-preserving insertion order as a stable tie-breaker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptionTrack {
    /// Subtitle body URL on `https://www.youtube.com`.
    pub base_url: Url,
    /// ISO 639-1 (or BCP-47) code, e.g. `"pt"`, `"en"`, `"pt-BR"`.
    pub language_code: String,
    /// Human-readable name, e.g. `"Portuguese (auto-generated)"`.
    pub name: String,
    /// Internal `vssId` (e.g. `".pt"` or `"a.pt"`).
    pub vss_id: String,
    /// `Some("asr")` for auto-generated, `None` for manual uploads.
    pub kind: Option<String>,
}

/// Subset of the `YouTube` `captionTrack` JSON we depend on.
///
/// `baseUrl` is required; missing fields default to empty strings so a
/// track that is structurally present still parses. `languageCode` is
/// required and an empty value surfaces as
/// [`AppError::LanguageParseError`].
#[derive(Debug, Deserialize)]
pub(crate) struct RawCaptionTrack {
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    #[serde(rename = "languageCode")]
    pub language_code: String,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "vssId", default)]
    pub vss_id: String,
    #[serde(default)]
    pub kind: Option<String>,
}

impl TryFrom<&serde_json::Value> for CaptionTrack {
    type Error = AppError;

    fn try_from(value: &serde_json::Value) -> Result<Self, Self::Error> {
        let raw: RawCaptionTrack =
            serde_json::from_value(value.clone()).map_err(AppError::Serde)?;
        if raw.base_url.is_empty() {
            return Err(AppError::Internal(
                "caption track missing baseUrl".to_string(),
            ));
        }
        if raw.language_code.is_empty() {
            return Err(AppError::LanguageParseError(
                "caption track has empty languageCode".to_string(),
            ));
        }
        let base_url = Url::parse(&raw.base_url)
            .map_err(|e| AppError::Internal(format!("caption track baseUrl parse failed: {e}")))?;
        Ok(Self {
            base_url,
            language_code: raw.language_code,
            name: raw.name,
            vss_id: raw.vss_id,
            kind: raw.kind,
        })
    }
}

/// Pick the best [`CaptionTrack`] for a requested language.
///
/// Selection rules (in order):
/// 1. Tracks whose normalised language matches `requested` (BCP-47
///    region tags like `pt-BR` collapse to `pt`).
/// 2. Within the matched set, prefer `kind == Some("asr")` when
///    `prefer_asr` is true (default false; the user opts in with
///    `--asr`).
/// 3. Manual tracks win when `prefer_asr` is false.
/// 4. Final tie-breaker: original array order (insertion order
///    preserved by the iterator).
pub fn select_track<'a, I>(tracks: I, requested: &str, prefer_asr: bool) -> Option<&'a CaptionTrack>
where
    I: IntoIterator<Item = &'a CaptionTrack>,
{
    let needle = normalise_lang(requested);
    let mut matches: Vec<&CaptionTrack> = tracks
        .into_iter()
        .filter(|t| normalise_lang(&t.language_code) == needle)
        .collect();
    if matches.is_empty() {
        return None;
    }
    matches.sort_by_key(|t| score(t, prefer_asr));
    matches.into_iter().next()
}

fn score(track: &CaptionTrack, prefer_asr: bool) -> u8 {
    let is_asr = matches!(track.kind.as_deref(), Some("asr"));
    match (prefer_asr, is_asr) {
        (true, true) => 0,
        (true, false) => 1,
        (false, false) => 0,
        (false, true) => 1,
    }
}

/// Reduce a BCP-47-style tag to its ISO 639-1 head: `pt-BR` -> `pt`,
/// `en-US` -> `en`, `pt` -> `pt`. Empty input returns empty.
fn normalise_lang(code: &str) -> String {
    code.split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn track(json: serde_json::Value) -> CaptionTrack {
        CaptionTrack::try_from(&json).expect("track should parse")
    }

    #[test]
    fn parses_minimal_track() {
        let raw = json!({
            "baseUrl": "https://www.youtube.com/api/timedtext?v=abc&lang=pt",
            "languageCode": "pt",
            "name": "Portuguese",
            "vssId": ".pt",
        });
        let t = track(raw);
        assert_eq!(t.language_code, "pt");
        assert_eq!(t.name, "Portuguese");
        assert_eq!(t.vss_id, ".pt");
        assert_eq!(t.kind, None);
    }

    #[test]
    fn parses_asr_kind() {
        let raw = json!({
            "baseUrl": "https://example.com/srv3?lang=pt",
            "languageCode": "pt",
            "name": "Portuguese (auto-generated)",
            "vssId": "a.pt",
            "kind": "asr",
        });
        let t = track(raw);
        assert_eq!(t.kind.as_deref(), Some("asr"));
    }

    #[test]
    fn rejects_empty_language_code() {
        let raw = json!({
            "baseUrl": "https://example.com",
            "languageCode": "",
            "name": "x",
        });
        let err = CaptionTrack::try_from(&raw).unwrap_err();
        assert!(matches!(err, AppError::LanguageParseError(_)));
    }

    #[test]
    fn rejects_empty_base_url() {
        let raw = json!({
            "baseUrl": "",
            "languageCode": "pt",
        });
        let err = CaptionTrack::try_from(&raw).unwrap_err();
        assert!(matches!(err, AppError::Internal(_)));
    }

    #[test]
    fn prefers_asr_when_requested() {
        let manual = track(json!({
            "baseUrl": "https://example.com/manual",
            "languageCode": "pt",
            "name": "Portuguese",
            "vssId": ".pt",
        }));
        let asr = track(json!({
            "baseUrl": "https://example.com/asr",
            "languageCode": "pt",
            "name": "Portuguese (auto-generated)",
            "vssId": "a.pt",
            "kind": "asr",
        }));
        let chosen = select_track(vec![&manual, &asr], "pt", true).expect("track expected");
        assert_eq!(chosen.base_url.as_str(), "https://example.com/asr");
    }

    #[test]
    fn falls_back_to_manual_when_no_asr() {
        let manual = track(json!({
            "baseUrl": "https://example.com/manual",
            "languageCode": "en",
            "name": "English",
            "vssId": ".en",
        }));
        let chosen = select_track(vec![&manual], "en", true).expect("track expected");
        assert_eq!(chosen.base_url.as_str(), "https://example.com/manual");
    }

    #[test]
    fn manual_wins_when_prefer_asr_false() {
        let manual = track(json!({
            "baseUrl": "https://example.com/manual",
            "languageCode": "pt",
            "name": "Portuguese",
            "vssId": ".pt",
        }));
        let asr = track(json!({
            "baseUrl": "https://example.com/asr",
            "languageCode": "pt",
            "name": "Portuguese (auto-generated)",
            "vssId": "a.pt",
            "kind": "asr",
        }));
        let chosen = select_track(vec![&asr, &manual], "pt", false).expect("track expected");
        assert_eq!(chosen.base_url.as_str(), "https://example.com/manual");
    }

    #[test]
    fn language_normalisation_collapses_bcp47() {
        assert_eq!(normalise_lang("pt-BR"), "pt");
        assert_eq!(normalise_lang("en_US"), "en");
        assert_eq!(normalise_lang("PT"), "pt");
        assert_eq!(normalise_lang(""), "");
    }

    #[test]
    fn select_track_returns_none_for_miss() {
        let t = track(json!({
            "baseUrl": "https://example.com/x",
            "languageCode": "pt",
        }));
        assert!(select_track(vec![&t], "en", false).is_none());
    }
}
