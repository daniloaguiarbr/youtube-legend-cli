//! Second concrete [`Provider`]: scrapes an HTML page for `var sid/hash/...`
//! tokens, encrypts the request body, and posts to the documented API.

use async_trait::async_trait;
use chrono::Utc;
use regex::Regex;
use reqwest::Client;
use scraper::{Html, Selector};
use std::time::Duration;

use super::{Format, Provider, SubtitleInfo};
use crate::crypto::encrypt_token;
use crate::error::{AppError, AppResult, NoSubtitleReason};
use crate::secret_endpoints::{
    COOKIE_ANTI_BOT_NAME, PROVIDER_B_API_PATH, PROVIDER_B_PRIMARY_HOST, PROVIDER_B_PRIMARY_PAGE,
    USER_AGENT_IDENTITY,
};

/// Provider B. Construct with [`ProviderB::new`] for the default
/// User-Agent, or with [`ProviderB::with_user_agent`] to override.
pub struct ProviderB {
    client: Client,
    language: Option<String>,
}

impl ProviderB {
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
            .referer(true)
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

    async fn fetch_page(&self, video_url: &str) -> AppResult<PageVars> {
        let url = format!("{PROVIDER_B_PRIMARY_PAGE}{}", urlencoding(video_url));
        let resp = self
            .client
            .get(&url)
            .header("Host", PROVIDER_B_PRIMARY_HOST)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
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
        let html = resp.text().await.map_err(AppError::Http)?;
        parse_page_vars(&html)
    }

    async fn post_api(&self, vars: &PageVars, video_url: &str) -> AppResult<String> {
        let chrono_ms = Utc::now().timestamp_millis();
        let plaintext = format!("{video_url};;{chrono_ms}");
        let token = encrypt_token(&plaintext)?;

        let tutoken = vars.tutoken.as_deref().unwrap_or("");
        let htoken = vars.htoken.as_deref().unwrap_or("");

        let form = [
            ("token", token.as_str()),
            ("sid", vars.sid.as_str()),
            ("hash", vars.hash.as_str()),
            ("hl", vars.hl.as_str()),
            ("tutoken", tutoken),
            ("htoken", htoken),
        ];

        let api_path = vars.api_path.as_deref().unwrap_or(PROVIDER_B_API_PATH);
        let resp = self
            .client
            .post(format!(
                "https://{}{}",
                PROVIDER_B_PRIMARY_HOST.trim_end_matches('/'),
                api_path
            ))
            .header("Host", PROVIDER_B_PRIMARY_HOST)
            .header("X-Requested-With", "XMLHttpRequest")
            .header("Accept", "*/*")
            .header("Cookie", format!("{COOKIE_ANTI_BOT_NAME}=0"))
            .form(&form)
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

        let html = resp.text().await.map_err(AppError::Http)?;
        if html.contains("cf-turnstile") || html.contains("h-captcha") {
            return Err(AppError::ProviderUnavailable);
        }

        if html.contains("failmsg") {
            return Err(AppError::NoSubtitle(
                crate::error::NoSubtitleReason::NotPublished,
            ));
        }

        Ok(html)
    }

    fn extract_download_link(&self, html: &str) -> Option<String> {
        use crate::secret_endpoints::PROVIDER_B_REDIRECT_HOST;
        let redirect_re = format!(
            r#"https?://{}/[^"'\s<>]+"#,
            regex::escape(PROVIDER_B_REDIRECT_HOST)
        );
        if let Ok(re) = Regex::new(&redirect_re) {
            if let Some(caps) = re.captures(html) {
                return Some(caps.get(0)?.as_str().to_string());
            }
        }
        if let Ok(re) = Regex::new(r#"https?://[^\s"'<>]+\.(srt|txt|vtt)"#) {
            if let Some(caps) = re.captures(html) {
                return Some(caps.get(0)?.as_str().to_string());
            }
        }
        let doc = Html::parse_document(html);
        let sel = Selector::parse("a[href]").ok()?;
        for el in doc.select(&sel) {
            if let Some(href) = el.value().attr("href") {
                if href.contains(PROVIDER_B_REDIRECT_HOST)
                    || href.contains(".srt")
                    || href.contains(".txt")
                {
                    return Some(href.to_string());
                }
            }
        }
        None
    }
}

/// Tokens extracted from a provider-B HTML page.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct PageVars {
    /// Original `YouTube` URL echoed back by the page.
    pub url: String,
    /// `var sid=...`: the obfuscated session id
    pub sid: String,
    /// `var hl=...`
    pub hl: String,
    /// `var hash=...`
    pub hash: String,
    /// `var tutoken='...'` when present.
    pub tutoken: Option<String>,
    /// `var htoken='...'` when present.
    pub htoken: Option<String>,
    /// AJAX endpoint path discovered from the page's inline JavaScript,
    /// e.g. `/api.php`. `None` falls back to the compiled-in
    /// `PROVIDER_B_API_PATH`. Discovering it at runtime lets the client
    /// adapt when the provider renames the endpoint.
    pub api_path: Option<String>,
}

fn parse_page_vars(html: &str) -> AppResult<PageVars> {
    let doc = Html::parse_document(html);
    let script_sel = Selector::parse("script")
        .map_err(|e| AppError::Internal(format!("selector parse failed: {e:?}")))?;

    let mut vars = PageVars::default();
    for el in doc.select(&script_sel) {
        let text = el.text().collect::<String>();
        for (key, dest) in [
            ("var url", &mut vars.url),
            ("var sid", &mut vars.sid),
            ("var hl", &mut vars.hl),
            ("var hash", &mut vars.hash),
        ] {
            if text.contains(key) {
                if let Some(value) = extract_var(&text, key) {
                    *dest = value;
                }
            }
        }
    }

    if let Ok(tutoken_re) = Regex::new(r"var tutoken='([a-f0-9]{32})'") {
        for el in doc.select(&script_sel) {
            let text = el.text().collect::<String>();
            if let Some(m) = tutoken_re.captures(&text).and_then(|c| c.get(1)) {
                vars.tutoken = Some(m.as_str().to_string());
                break;
            }
        }
    }
    if let Ok(htoken_re) = Regex::new(r"var htoken='([a-f0-9]{32})'") {
        for el in doc.select(&script_sel) {
            let text = el.text().collect::<String>();
            if let Some(m) = htoken_re.captures(&text).and_then(|c| c.get(1)) {
                vars.htoken = Some(m.as_str().to_string());
                break;
            }
        }
    }

    vars.api_path = discover_api_path(html);

    if vars.sid.is_empty() || vars.hash.is_empty() || vars.hl.is_empty() {
        return Err(AppError::ProviderUnavailable);
    }

    Ok(vars)
}

/// Discover the provider-B AJAX endpoint path from the page's inline
/// JavaScript. The site references its API path as a quoted string
/// (for example `"/api.php"`). Extracting it at runtime lets the client
/// adapt when the provider renames the endpoint, instead of relying
/// solely on the compiled-in `PROVIDER_B_API_PATH` fallback. A path
/// containing `api` is preferred; otherwise the first `.php` path wins.
fn discover_api_path(html: &str) -> Option<String> {
    let re = Regex::new(r#"["'](/[A-Za-z0-9_./-]*\.php)["']"#).ok()?;
    let mut first: Option<String> = None;
    for caps in re.captures_iter(html) {
        if let Some(m) = caps.get(1) {
            let path = m.as_str().to_string();
            if path.contains("api") {
                return Some(path);
            }
            first.get_or_insert(path);
        }
    }
    first
}

fn extract_var(text: &str, key: &str) -> Option<String> {
    let start = text.find(key)?;
    let rest = &text[start + key.len()..];
    let after_eq = rest.find('=')?;
    let value_start = rest[after_eq + 1..].find(|c: char| !c.is_whitespace())?;
    let from = after_eq + 1 + value_start;
    let remainder = &rest[from..];
    let terminator = remainder.find([';', '\n']).unwrap_or(remainder.len());
    let value = remainder[..terminator].trim();
    if (value.starts_with('"') && value.ends_with('"') && value.len() >= 2)
        || (value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2)
    {
        Some(value[1..value.len() - 1].to_string())
    } else {
        Some(value.to_string())
    }
}

#[async_trait]
impl Provider for ProviderB {
    fn name(&self) -> &'static str {
        "provider-b"
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
        super::robots::check_allowed(PROVIDER_B_PRIMARY_HOST, "/", USER_AGENT_IDENTITY).await?;
        tracing::info!(
            target: "events",
            provider = "provider-b",
            video_id,
            language = %lang_filter,
            "fetch_subtitle_started"
        );
        let vars = self.fetch_page(&video_url).await?;
        let html = self.post_api(&vars, &video_url).await?;
        let download_url = self
            .extract_download_link(&html)
            .ok_or(AppError::NoSubtitle(
                crate::error::NoSubtitleReason::NotPublished,
            ))?;
        tracing::info!(
            target: "events",
            provider = "provider-b",
            video_id,
            source_url = %download_url,
            "fetch_subtitle_completed"
        );
        Ok(SubtitleInfo {
            video_id: video_id.to_string(),
            language: lang_filter,
            format,
            source_url: download_url,
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

#[cfg(test)]
mod url_tests {
    use crate::secret_endpoints::{PROVIDER_B_API_PATH, PROVIDER_B_PRIMARY_HOST};

    /// Regression guard for the provider-B API POST URL. The host
    /// constant is a bare authority (no scheme), so the URL must be
    /// built with an explicit `https://` prefix; otherwise reqwest
    /// rejects it at send time with an opaque "builder error" and the
    /// request never leaves the process. This asserts the constructed
    /// URL parses as an absolute HTTPS URL without revealing the value.
    #[test]
    fn api_post_url_is_a_valid_absolute_https_url() {
        let url = format!(
            "https://{}{}",
            PROVIDER_B_PRIMARY_HOST.trim_end_matches('/'),
            PROVIDER_B_API_PATH
        );
        let parsed = reqwest::Url::parse(&url).expect("API URL must parse");
        assert_eq!(parsed.scheme(), "https");
        assert!(parsed.host().is_some(), "API URL must have a host");
    }

    #[test]
    fn discover_api_path_finds_php_endpoint() {
        let html = r#"<script>var a=["x","/api.php","post"];$.post("/api.php",d)</script>"#;
        assert_eq!(super::discover_api_path(html).as_deref(), Some("/api.php"));
    }

    #[test]
    fn discover_api_path_prefers_api_named_path() {
        let html = r#"<script>track("/log.php"); call("/api2.php");</script>"#;
        assert_eq!(super::discover_api_path(html).as_deref(), Some("/api2.php"));
    }

    #[test]
    fn discover_api_path_none_when_absent() {
        assert_eq!(
            super::discover_api_path("<html>no scripts here</html>"),
            None
        );
    }
}
