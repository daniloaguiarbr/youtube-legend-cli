//! Direct `YouTube` provider that talks to the public watch-page
//! endpoint without going through any third-party index.
//!
//! This is the M1 skeleton. M2 (Srv3/JSON3 conversion), M3
//! (signature decipher), and M3.5 (n-parameter) will be added on top
//! without changing the public trait surface. The skeleton already
//! does the work that requires no decipher:
//!
//! 1. Fetch the watch-page HTML with a `reqwest::Client`.
//! 2. Parse the embedded `ytInitialPlayerResponse` JSON.
//! 3. Select a caption track that matches the requested language,
//!    preferring ASR when the user opted in via `--asr`.
//! 4. Return a [`SubtitleInfo`] pointing at the track's `baseUrl`.
//!
//! `fetch_content` is left as a stub because in M1 we have not yet
//! decided how to convert JSON3 / Srv3 / Srv1 bodies into SRT. The
//! stub is loud enough to fail closed — operators see an explicit
//! "not implemented in M1" instead of an empty body that gets
//! silently promoted as success.

use async_trait::async_trait;
use reqwest::header::CONTENT_TYPE;
use reqwest::Client;
use std::time::Duration;
use url::Url;

use super::youtube::caption_track::{select_track, CaptionTrack};
use super::youtube::player_response::{fetch_player_response, DEFAULT_MAX_BODY_BYTES};
use super::{Format, Provider, SubtitleInfo};
use crate::error::{AppError, AppResult, NoSubtitleReason};
use crate::parse::srv3::{json3_to_srt, srv3_to_srt, MAX_BODY_BYTES};
use crate::secret_endpoints::USER_AGENT_IDENTITY;

/// Direct `YouTube` provider. Constructed via [`ProviderYouTubeDirect::new`]
/// or [`ProviderYouTubeDirect::with_user_agent`].
pub struct ProviderYouTubeDirect {
    client: Client,
    user_agent: String,
    prefer_asr: bool,
    max_body_bytes: usize,
}

impl ProviderYouTubeDirect {
    /// Build a provider with the built-in `User-Agent`.
    ///
    /// # Errors
    ///
    /// - [`AppError::Http`] when the underlying `reqwest` client fails
    ///   to build.
    #[tracing::instrument(level = "debug", err)]
    pub fn new() -> AppResult<Self> {
        Self::with_user_agent(USER_AGENT_IDENTITY)
    }

    /// Build a provider with a custom `User-Agent`.
    ///
    /// # Errors
    ///
    /// - [`AppError::Http`] when the underlying `reqwest` client fails
    ///   to build.
    #[tracing::instrument(level = "debug", err, skip(user_agent))]
    pub fn with_user_agent(user_agent: &str) -> AppResult<Self> {
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(5))
            .gzip(true)
            .build()
            .map_err(AppError::Http)?;
        Ok(Self {
            client,
            user_agent: user_agent.to_string(),
            prefer_asr: false,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
        })
    }

    /// Builder-style: opt into auto-generated captions (ASR) when
    /// picking among multiple tracks in the requested language.
    pub fn prefer_asr(mut self, value: bool) -> Self {
        self.prefer_asr = value;
        self
    }

    /// Builder-style: override the watch-page body cap. Mostly useful
    /// in tests; production operators leave the default.
    pub fn with_max_body_bytes(mut self, bytes: usize) -> Self {
        self.max_body_bytes = bytes;
        self
    }

    /// Currently configured language preference for ASR.
    pub fn prefers_asr(&self) -> bool {
        self.prefer_asr
    }

    async fn find_track(&self, video_id: &str, language: &str) -> AppResult<CaptionTrack> {
        let response = fetch_player_response(&self.client, video_id, self.max_body_bytes).await?;
        if response.caption_tracks.is_empty() {
            return Err(AppError::NoSubtitle(NoSubtitleReason::NotPublished));
        }
        let chosen = select_track(&response.caption_tracks, language, self.prefer_asr)
            .ok_or(AppError::NoSubtitle(NoSubtitleReason::LanguageUnavailable))?;
        Ok(chosen.clone())
    }

    async fn fetch_and_convert(&self, url: &str, fmt: SubtitleFmt) -> AppResult<Vec<u8>> {
        match fmt {
            SubtitleFmt::Srv1 => Err(AppError::Internal("srv1 not implemented in M2".to_string())),
            SubtitleFmt::Srv3 | SubtitleFmt::Json3 => {
                let body = fetch_body(&self.client, url, MAX_BODY_BYTES).await?;
                let srt = match fmt {
                    SubtitleFmt::Srv3 => srv3_to_srt(&body)?,
                    SubtitleFmt::Json3 => json3_to_srt(&body)?,
                    SubtitleFmt::Srv1 => unreachable!(),
                };
                Ok(srt.into_bytes())
            }
        }
    }
}

#[async_trait]
impl Provider for ProviderYouTubeDirect {
    fn name(&self) -> &'static str {
        "youtube-direct"
    }

    #[tracing::instrument(level = "debug", err, skip(self), fields(video_id, language, format = ?format, prefer_asr = self.prefer_asr, user_agent = %self.user_agent))]
    async fn fetch_subtitle(
        &self,
        video_id: &str,
        language: &str,
        format: Format,
    ) -> AppResult<SubtitleInfo> {
        let track = self.find_track(video_id, language).await?;
        let source_url = track.base_url.to_string();
        // M3.5 detection: surface a structured event when the
        // baseUrl carries an `n=` query parameter. The actual
        // ncode application lives behind the M3 player.js
        // pipeline (see `player_js::extract_n_code` and
        // `youtube::ncode::ncode`); here we only flag the case so
        // operators can confirm the integration end-to-end.
        let has_n_param = Url::parse(&source_url)
            .ok()
            .and_then(|u| {
                u.query_pairs()
                    .find(|(k, _)| k == "n")
                    .map(|(_, v)| v.into_owned())
            })
            .is_some();
        if has_n_param {
            tracing::info!(
                target: "youtube_decipher",
                provider = "youtube-direct",
                video_id,
                "baseUrl carries n-parameter; ncode path will apply at fetch_content"
            );
        }
        tracing::info!(
            target: "events",
            provider = "youtube-direct",
            video_id,
            language = %track.language_code,
            kind = track.kind.as_deref().unwrap_or("manual"),
            source_url = %source_url,
            has_n_param,
            "fetch_subtitle_completed"
        );
        Ok(SubtitleInfo {
            video_id: video_id.to_string(),
            language: track.language_code,
            format,
            source_url,
            byte_size: 0,
        })
    }

    async fn fetch_content(&self, info: &SubtitleInfo) -> AppResult<Vec<u8>> {
        // The  we stashed in M1 carries the  query
        // parameter ( or ). We always try  first
        // because the wire payload is the most common shape served
        // by  today, then fall back to , then
        // .  is binary protobuf; M2 only implements the
        // text formats, so a  base URL returns an explicit
        // internal error so callers can tell "supported fmt missing"
        // apart from a genuine empty body.
        let fmts = detect_fmt_candidates(&info.source_url);
        let mut last_err: Option<AppError> = None;
        for fmt in fmts {
            let url = match apply_fmt(&info.source_url, fmt) {
                Ok(url) => url,
                Err(e) => {
                    last_err = Some(e);
                    continue;
                }
            };
            match self.fetch_and_convert(&url, fmt).await {
                Ok(bytes) => return Ok(bytes),
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err
            .unwrap_or_else(|| AppError::Internal("no fmt candidate produced a body".to_string())))
    }
}

/// Wire format of a `fmt=` parameter on a `YouTube` timed-text URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubtitleFmt {
    /// Binary protobuf (`fmt=srv1`). Not implemented in M2.
    Srv1,
    /// Flat XML (`fmt=srv3`).
    Srv3,
    /// Nested JSON (`fmt=json3`).
    Json3,
}

impl SubtitleFmt {
    /// String token used in the `fmt` query parameter.
    fn as_token(self) -> &'static str {
        match self {
            SubtitleFmt::Srv1 => "srv1",
            SubtitleFmt::Srv3 => "srv3",
            SubtitleFmt::Json3 => "json3",
        }
    }
}

/// Build the ordered list of formats to try for a given `baseUrl`.
///
/// The URL's existing `fmt` parameter (if any) is tried first so the
/// upstream default is preserved. Then `json3`, then `srv3`, then
/// `srv1`. `srv1` is included as a last-ditch candidate so the
/// caller gets a structured Internal error instead of silently
/// failing when only the binary payload is available.
fn detect_fmt_candidates(base_url: &str) -> Vec<SubtitleFmt> {
    let mut out: Vec<SubtitleFmt> = Vec::with_capacity(4);
    if let Ok(parsed) = Url::parse(base_url) {
        for (k, v) in parsed.query_pairs() {
            if k == "fmt" {
                match v.as_ref() {
                    "srv1" => out.push(SubtitleFmt::Srv1),
                    "srv3" => out.push(SubtitleFmt::Srv3),
                    "json3" => out.push(SubtitleFmt::Json3),
                    _ => {}
                }
                break;
            }
        }
    }
    for fmt in [SubtitleFmt::Json3, SubtitleFmt::Srv3, SubtitleFmt::Srv1] {
        if !out.contains(&fmt) {
            out.push(fmt);
        }
    }
    out
}

/// Replace (or insert) the `fmt` query parameter on a `baseUrl`,
/// returning a fully-encoded URL string.
fn apply_fmt(base_url: &str, fmt: SubtitleFmt) -> AppResult<String> {
    let token = fmt.as_token();
    // We rebuild the query string ourselves because the `url` crate
    // v2.5 Serializer API does not expose `set_pair`. Pairing it back
    // through `Url::parse` keeps the rest of the path (and any
    // signature/n-parameter from M3) byte-for-byte stable.
    let parsed = Url::parse(base_url).map_err(AppError::UrlParse)?;
    let mut new_pairs: Vec<(String, String)> = Vec::new();
    let mut found = false;
    for (k, v) in parsed.query_pairs() {
        if k == "fmt" {
            new_pairs.push((k.into_owned(), token.to_string()));
            found = true;
        } else {
            new_pairs.push((k.into_owned(), v.into_owned()));
        }
    }
    if !found {
        new_pairs.push(("fmt".to_string(), token.to_string()));
    }
    let mut url = parsed.clone();
    url.set_query(None);
    {
        let mut ser = url.query_pairs_mut();
        for (k, v) in &new_pairs {
            ser.append_pair(k, v);
        }
    }
    Ok(url.to_string())
}

/// Fetch a timed-text body, enforcing a hard cap on bytes and
/// rejecting non-XML/non-JSON `Content-Type` for the matching fmt.
async fn fetch_body(client: &Client, url: &str, max_body_bytes: usize) -> AppResult<String> {
    let response = client.get(url).send().await.map_err(AppError::Http)?;
    let status = response.status();
    if !status.is_success() {
        return Err(AppError::ProviderUnavailable);
    }
    if let Some(len) = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok())
    {
        if len > max_body_bytes {
            return Err(AppError::SubtitleTooLarge(len));
        }
    }
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let body = response.text().await.map_err(AppError::Http)?;
    if body.len() > max_body_bytes {
        return Err(AppError::SubtitleTooLarge(body.len()));
    }
    if !content_type.is_empty()
        && !content_type.contains("xml")
        && !content_type.contains("json")
        && !content_type.contains("text/plain")
    {
        return Err(AppError::InvalidInput(format!(
            "unexpected content-type {content_type:?} for timed-text body"
        )));
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_sets_prefer_asr() {
        let p = ProviderYouTubeDirect::new()
            .expect("client builds")
            .prefer_asr(true);
        assert!(p.prefers_asr());
    }

    #[test]
    fn builder_overrides_max_body_bytes() {
        let p = ProviderYouTubeDirect::new()
            .expect("client builds")
            .with_max_body_bytes(2048);
        assert_eq!(p.max_body_bytes, 2048);
    }

    #[test]
    fn name_is_youtube_direct() {
        let p = ProviderYouTubeDirect::new().expect("client builds");
        assert_eq!(p.name(), "youtube-direct");
    }
}
