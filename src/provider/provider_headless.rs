//! Headless-browser fallback [`Provider`].
//!
//! Drives a real Chromium/Chrome instance to run the upstream site's
//! own JavaScript, then downloads the subtitle through the page's
//! same-origin session. This is the only path that works when the
//! upstream endpoint is gated behind Cloudflare and requires
//! browser-executed JS: the plain HTTP providers receive `404`/`403`
//! because the download endpoints reject non-browser requests.
//!
//! Gated behind the `headless` Cargo feature because it depends on a
//! local Chromium/Chrome install at runtime.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use chromiumoxide::{Browser, BrowserConfig};
use futures::StreamExt;

use super::{Format, Provider, SubtitleInfo};
use crate::error::{AppError, AppResult, NoSubtitleReason};
use crate::secret_endpoints::PROVIDER_B_PRIMARY_PAGE;

/// JavaScript run in the result page. Collects the per-language download
/// buttons (each carries a `data-href` to the real download endpoint),
/// picks the one matching the requested format and language (falling
/// back to the auto-generated `a.<lang>` variant, then to the first
/// available), fetches it on the page's own origin, and returns the
/// body. `__FMT__` and `__LANG__` are substituted before evaluation.
const DOWNLOAD_JS: &str = r#"(async()=>{
  const all=Array.from(document.querySelectorAll("a")).filter(e=>e.dataset.href&&/get2\.php/.test(e.dataset.href));
  if(!all.length) return JSON.stringify({err:"no buttons"});
  const fmt="__FMT__", lang="__LANG__";
  const hlOf=h=>((h.match(/hl=([^&]*)/)||[])[1]||"");
  const hasFmt=e=>new RegExp("format="+fmt).test(e.dataset.href);
  let btn=all.find(e=>hasFmt(e)&&hlOf(e.dataset.href)===lang)
       || all.find(e=>hasFmt(e)&&hlOf(e.dataset.href)==="a."+lang)
       || all.find(e=>hasFmt(e));
  if(!btn) return JSON.stringify({err:"no matching button",langs:[...new Set(all.map(e=>hlOf(e.dataset.href)))]});
  const r=await fetch(btn.dataset.href);
  const body=await r.text();
  return JSON.stringify({lang:hlOf(btn.dataset.href),status:r.status,body});
})()"#;

/// Headless-browser provider. Construct with [].
#[cfg(feature = "headless")]
pub struct ProviderHeadless {
    language: Option<String>,
    /// Subtitle bodies fetched during `fetch_subtitle`, keyed by the
    /// synthetic `source_url`, returned later by `fetch_content`.
    cache: Mutex<HashMap<String, Vec<u8>>>,
}

impl Default for ProviderHeadless {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderHeadless {
    /// Build a new headless provider.
    #[tracing::instrument(level = "debug")]
    pub fn new() -> Self {
        Self {
            language: None,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Builder-style: pin a language filter for all subsequent calls.
    #[tracing::instrument(level = "debug", skip(self))]
    pub fn with_language(mut self, language: &str) -> Self {
        self.language = Some(language.to_string());
        self
    }

    /// Resolve a Chromium/Chrome executable: honour `$CHROME`, then try
    /// well-known paths. Returns `None` when no browser is installed.
    fn chrome_path() -> Option<String> {
        if let Ok(p) = std::env::var("CHROME") {
            if !p.is_empty() {
                return Some(p);
            }
        }
        const CANDIDATES: &[&str] = &[
            "/usr/bin/chromium-browser",
            "/usr/bin/chromium",
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/brave-browser",
        ];
        CANDIDATES
            .iter()
            .find(|c| std::path::Path::new(c).exists())
            .map(|c| c.to_string())
    }
}

/// Percent-encode a URL for the `?u=` query parameter.
fn urlencode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Base language code: `pt-BR` and `pt_BR.UTF-8` both reduce to `pt`.
fn base_lang(language: &str) -> String {
    language
        .split(['-', '_', '.'])
        .next()
        .unwrap_or(language)
        .to_lowercase()
}

#[async_trait]
impl Provider for ProviderHeadless {
    fn name(&self) -> &'static str {
        "provider-headless"
    }

    async fn fetch_subtitle(
        &self,
        video_id: &str,
        language: &str,
        format: Format,
    ) -> AppResult<SubtitleInfo> {
        let lang = self
            .language
            .clone()
            .unwrap_or_else(|| language.to_string());
        let lang_base = base_lang(&lang);
        let fmt = match format {
            Format::Srt => "srt",
            Format::Txt => "txt",
        };

        tracing::info!(
            target: "events",
            provider = "provider-headless",
            video_id,
            language = %lang_base,
            "fetch_subtitle_started"
        );

        let chrome = Self::chrome_path().ok_or(AppError::ProviderUnavailable)?;
        let config = BrowserConfig::builder()
            .chrome_executable(chrome)
            .arg("--no-sandbox")
            .arg("--disable-gpu")
            .arg("--disable-dev-shm-usage")
            .build()
            .map_err(|e| AppError::Internal(format!("browser config: {e}")))?;
        let (mut browser, mut handler) = Browser::launch(config)
            .await
            .map_err(|_| AppError::ProviderUnavailable)?;
        let handler_task = tokio::spawn(async move { while handler.next().await.is_some() {} });

        let outcome = drive_page(&browser, video_id, fmt, &lang_base).await;

        let _ = browser.close().await;
        handler_task.abort();

        let (matched_lang, body) = outcome?;
        let source_url = format!("headless://{video_id}/{matched_lang}/{fmt}");
        self.cache
            .lock()
            .map_err(|_| AppError::Internal("cache poisoned".into()))?
            .insert(source_url.clone(), body.into_bytes());

        tracing::info!(
            target: "events",
            provider = "provider-headless",
            video_id,
            language = %matched_lang,
            "fetch_subtitle_completed"
        );

        Ok(SubtitleInfo {
            video_id: video_id.to_string(),
            language: matched_lang,
            format,
            source_url,
            byte_size: 0,
        })
    }

    async fn fetch_content(&self, info: &SubtitleInfo) -> AppResult<Vec<u8>> {
        let bytes = self
            .cache
            .lock()
            .map_err(|_| AppError::Internal("cache poisoned".into()))?
            .get(&info.source_url)
            .cloned();
        match bytes {
            Some(b) if !b.is_empty() => Ok(b),
            _ => Err(AppError::NoSubtitle(NoSubtitleReason::NotPublished)),
        }
    }
}

/// Navigate to the result page, click the extract button, run the
/// download JS, and return `(matched_language, body)`.
async fn drive_page(
    browser: &Browser,
    video_id: &str,
    fmt: &str,
    lang_base: &str,
) -> AppResult<(String, String)> {
    let video_url = format!("https://www.youtube.com/watch?v={video_id}");
    let page_url = format!("{PROVIDER_B_PRIMARY_PAGE}{}", urlencode(&video_url));
    let page = browser
        .new_page(&page_url)
        .await
        .map_err(|_| AppError::ProviderUnavailable)?;
    let _ = page.wait_for_navigation().await;
    tokio::time::sleep(Duration::from_secs(3)).await;

    if let Ok(el) = page.find_element("#getsubtitle").await {
        let _ = el.click().await;
    }
    tokio::time::sleep(Duration::from_secs(6)).await;

    let js = DOWNLOAD_JS
        .replace("__FMT__", fmt)
        .replace("__LANG__", lang_base);
    let raw: String = page
        .evaluate(js)
        .await
        .map_err(|_| AppError::ProviderUnavailable)?
        .into_value()
        .map_err(|_| AppError::ProviderUnavailable)?;
    let _ = page.close().await;

    let v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|_| AppError::ProviderUnavailable)?;
    if v.get("err").is_some() {
        return Err(AppError::NoSubtitle(NoSubtitleReason::NotPublished));
    }
    if v["status"].as_u64() != Some(200) {
        return Err(AppError::NoSubtitle(NoSubtitleReason::NotPublished));
    }
    let body = v["body"].as_str().unwrap_or("").to_string();
    if body.is_empty() {
        return Err(AppError::NoSubtitle(NoSubtitleReason::NotPublished));
    }
    let matched = v["lang"].as_str().unwrap_or(lang_base).to_string();
    Ok((matched, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_lang_reduces_region_tags() {
        assert_eq!(base_lang("pt-BR"), "pt");
        assert_eq!(base_lang("pt_BR.UTF-8"), "pt");
        assert_eq!(base_lang("EN"), "en");
        assert_eq!(base_lang("es"), "es");
    }

    #[test]
    fn urlencode_escapes_reserved() {
        assert_eq!(
            urlencode("https://www.youtube.com/watch?v=abc"),
            "https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3Dabc"
        );
    }

    #[tokio::test]
    #[ignore = "live network + local Chromium: downloads a real subtitle via headless browser"]
    async fn headless_downloads_real_subtitle() {
        let provider = ProviderHeadless::new();
        let info = provider
            .fetch_subtitle("uqok8qe11wU", "pt", Format::Srt)
            .await
            .expect("fetch_subtitle");
        let body = provider.fetch_content(&info).await.expect("fetch_content");
        assert!(body.len() > 1000, "subtitle body too small: {}", body.len());
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("-->"), "not an SRT body");
    }
}
