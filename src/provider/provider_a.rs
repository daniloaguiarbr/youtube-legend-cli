//! First concrete [`Provider`]: scrapes an HTML index page, extracts a
//! token, and follows the documented JSON and `SubRip` URLs.

use async_trait::async_trait;
use reqwest::Client;
use scraper::{Html, Selector};
use serde_json::Value;
use std::time::Duration;

use super::{Format, Provider, SubtitleInfo};
use crate::error::{AppError, AppResult, NoSubtitleReason};
use crate::secret_endpoints::{
    PROVIDER_A_INFO_BASE, PROVIDER_A_PRIMARY_HOST, PROVIDER_A_PRIMARY_PAGE,
    PROVIDER_A_SUBTITLE_BASE, USER_AGENT_IDENTITY,
};

/// Provider A. Construct with [`ProviderA::new`] for the default
/// User-Agent, or with [`ProviderA::with_user_agent`] to override.
pub struct ProviderA {
    client: Client,
    language: Option<String>,
}

impl ProviderA {
    /// Build a new provider with the built-in `User-Agent`.
    ///
    /// # Errors
    ///
    /// - [`AppError::Http`] when the underlying `reqwest` client fails
    ///   to build.
    /// Build a new provider with the built-in `User-Agent`.
    ///
    /// # Errors
    ///
    /// - [`AppError::Http`] when the underlying `reqwest` client fails
    ///   to build.
    #[tracing::instrument(level = "debug", err)]
    pub fn new() -> AppResult<Self> {
        Self::with_user_agent(USER_AGENT_IDENTITY)
    }

    /// Build a new provider with a custom `User-Agent`.
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
            language: None,
        })
    }

    /// Builder-style: pin a language filter for all subsequent calls.
    #[tracing::instrument(level = "debug", skip(self))]
    pub fn with_language(mut self, language: &str) -> Self {
        self.language = Some(language.to_string());
        self
    }

    /// Currently configured language filter, if any.
    #[tracing::instrument(level = "debug", skip(self))]
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    async fn fetch_page_html(&self, video_url: &str) -> AppResult<String> {
        let url = format!("{PROVIDER_A_PRIMARY_PAGE}{}", urlencoding(video_url));
        let resp = self
            .client
            .get(&url)
            .header("Host", PROVIDER_A_PRIMARY_HOST)
            .send()
            .await
            .map_err(AppError::Http)?;
        let status = resp.status();
        if let Some(reason) = NoSubtitleReason::from_status(status.as_u16()) {
            return Err(AppError::NoSubtitle(reason));
        }
        if !status.is_success() {
            return Err(super::http_failure(status, resp.headers()));
        }
        resp.text().await.map_err(AppError::Http)
    }

    fn extract_token(&self, html: &str) -> Option<String> {
        let doc = Html::parse_document(html);
        let sel = Selector::parse("script").ok()?;
        for el in doc.select(&sel) {
            let text = el.text().collect::<String>();
            if let Some(idx) = text.find("/eyJ") {
                let rest = &text[idx + 1..];
                let end = rest
                    .find(['"', '\'', ' ', '\n', '\t'])
                    .unwrap_or(rest.len());
                return Some(rest[..end].to_string());
            }
        }
        // Fallback: parse JSON-LD `<script type="application/ld+json">`
        // blocks looking for a VideoObject with `caption` or `subtitles`
        // property pointing at a SubRip URL. Closes GAP-023.
        try_parse_json_ld(html)
    }

    async fn fetch_info_json(&self, token: &str) -> AppResult<Value> {
        let url = format!("{PROVIDER_A_INFO_BASE}{token}");
        let resp = self.client.get(&url).send().await.map_err(AppError::Http)?;
        let status = resp.status();
        if status.as_u16() == 400 {
            return Err(AppError::NoSubtitle(
                crate::error::NoSubtitleReason::NotPublished,
            ));
        }
        if let Some(reason) = NoSubtitleReason::from_status(status.as_u16()) {
            return Err(AppError::NoSubtitle(reason));
        }
        if !status.is_success() {
            return Err(super::http_failure(status, resp.headers()));
        }
        resp.json::<Value>().await.map_err(AppError::Http)
    }

    fn build_subtitle_url(&self, token: &str, format: Format) -> String {
        let prefix = match format {
            Format::Srt => "srt",
            Format::Txt => "txt",
        };
        format!("{PROVIDER_A_SUBTITLE_BASE}{prefix}/{token}")
    }
}

#[async_trait]
impl Provider for ProviderA {
    fn name(&self) -> &'static str {
        "provider-a"
    }

    #[tracing::instrument(level = "debug", err, skip(self), fields(video_id, language, format = ?format))]
    async fn fetch_subtitle(
        &self,
        video_id: &str,
        language: &str,
        format: Format,
    ) -> AppResult<SubtitleInfo> {
        let video_url = format!("https://www.youtube.com/watch?v={video_id}");
        let lang_filter = self
            .language
            .clone()
            .unwrap_or_else(|| language.to_string());
        // NFR-007: consult robots.txt for the index page before any
        // request. Failures are fail-open inside the helper.
        super::robots::check_allowed(PROVIDER_A_PRIMARY_HOST, "/", USER_AGENT_IDENTITY).await?;
        tracing::info!(
            target: "events",
            provider = "provider-a",
            video_id,
            language = %lang_filter,
            "fetch_subtitle_started"
        );
        let html = self.fetch_page_html(&video_url).await?;
        let token = self.extract_token(&html).ok_or(AppError::NoSubtitle(
            crate::error::NoSubtitleReason::NotPublished,
        ))?;

        let info = self.fetch_info_json(&token).await?;
        filter_language_match(&info, &lang_filter)?;

        let source_url = self.build_subtitle_url(&token, format);
        tracing::info!(
            target: "events",
            provider = "provider-a",
            video_id,
            source_url = %source_url,
            "fetch_subtitle_completed"
        );
        Ok(SubtitleInfo {
            video_id: video_id.to_string(),
            language: lang_filter,
            format,
            source_url,
            byte_size: 0,
        })
    }

    async fn fetch_content(&self, info: &SubtitleInfo) -> AppResult<Vec<u8>> {
        let resp = self
            .client
            .get(&info.source_url)
            .send()
            .await
            .map_err(AppError::Http)?;
        let status = resp.status();
        if let Some(reason) = NoSubtitleReason::from_status(status.as_u16()) {
            return Err(AppError::NoSubtitle(reason));
        }
        if !status.is_success() {
            return Err(super::http_failure(status, resp.headers()));
        }
        let bytes = resp.bytes().await.map_err(AppError::Http)?;
        if bytes.is_empty() {
            return Err(AppError::NoSubtitle(
                crate::error::NoSubtitleReason::NotPublished,
            ));
        }
        Ok(bytes.to_vec())
    }
}

fn urlencoding(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    out
}

fn filter_language_match(info: &Value, requested: &str) -> AppResult<()> {
    if let Some(array) = info.as_array() {
        for entry in array {
            if let Some(lang) = entry.get("lang").and_then(|v| v.as_str()) {
                if lang == requested {
                    return Ok(());
                }
            }
        }
    } else if let Some(obj) = info.as_object() {
        if let Some(lang) = obj.get("lang").and_then(|v| v.as_str()) {
            if lang == requested {
                return Ok(());
            }
        }
    }
    Err(AppError::NoSubtitle(
        crate::error::NoSubtitleReason::LanguageUnavailable,
    ))
}

/// Search HTML for `<script type="application/ld+json">` blocks, parse
/// each as JSON, and return the first URL string found in a
/// `VideoObject` `caption` or `subtitles` property. Returns `None`
/// when no candidate script is present, when JSON parsing fails, or
/// when the schema does not match `VideoObject` with a usable URL.
///
/// Closes GAP-023: third-party index pages occasionally embed
/// structured data pointing directly at the `SubRip` URL even when the
/// `/eyJ...` token is rendered via JavaScript that our scraper cannot
/// reach.
fn try_parse_json_ld(html: &str) -> Option<String> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(r#"script[type="application/ld+json"]"#).ok()?;
    for el in doc.select(&sel) {
        let text = el.text().collect::<String>();
        let value: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(url) = extract_video_object_url(&value) {
            return Some(url);
        }
    }
    None
}

fn extract_video_object_url(value: &Value) -> Option<String> {
    let mut queue = vec![value];
    while let Some(node) = queue.pop() {
        match node {
            Value::Object(map) => {
                let is_video = map
                    .get("@type")
                    .and_then(|t| t.as_str())
                    .map(|t| t == "VideoObject")
                    .unwrap_or(false);
                if is_video {
                    for key in ["caption", "subtitles"] {
                        if let Some(url) = map.get(key).and_then(|u| u.as_str()) {
                            return Some(url.to_string());
                        }
                        if let Some(arr) = map.get(key).and_then(|u| u.as_array()) {
                            for entry in arr {
                                if let Some(s) = entry.as_str() {
                                    return Some(s.to_string());
                                }
                                if let Some(obj) = entry.as_object() {
                                    if let Some(s) = obj.get("url").and_then(|u| u.as_str()) {
                                        return Some(s.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                for v in map.values() {
                    queue.push(v);
                }
            }
            Value::Array(arr) => {
                for v in arr {
                    queue.push(v);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_ld_videoobject_caption_fallback_yields_url() {
        let html = r#"
            <html>
              <head>
                <script type="application/ld+json">
                  {"@context":"https://schema.org","@type":"VideoObject",
                   "name":"Sample","caption":"https://example.com/eyJTOKEN.srt"}
                </script>
              </head>
              <body></body>
            </html>
        "#;
        let token = try_parse_json_ld(html);
        assert_eq!(token.as_deref(), Some("https://example.com/eyJTOKEN.srt"));
    }

    #[test]
    fn json_ld_missing_returns_none() {
        let html = "<html><body><p>no structured data here</p></body></html>";
        assert_eq!(try_parse_json_ld(html), None);
    }

    #[test]
    fn json_ld_wrong_type_returns_none() {
        let html = r#"
            <html><head>
              <script type="application/ld+json">
                {"@type":"Article","headline":"x"}
              </script>
            </head></html>
        "#;
        assert_eq!(try_parse_json_ld(html), None);
    }
}
