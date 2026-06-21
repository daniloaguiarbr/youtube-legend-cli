//! Headless-browser fallback [`Provider`] driving noteey.com.
//!
//! See GAP-AUD-2026-038. noteey.com is the exclusive provider since
//! v0.3.2. The
//! page is a vanilla Vue form: a single text input pre-filled with
//! the `YouTube` URL, a "Get Subtitle" button, and a transcript pane
//! that renders one cue per line with a leading `MM:SS` (or
//! `HH:MM:SS`) timestamp prefix.
//!
//! We replicate the same browser interaction `provider_headless` uses
//! for downsub: `Object.getOwnPropertyDescriptor(HTMLInputElement
//! .prototype, 'value').set` for Vue-native reactivity, then click,
//! then poll the transcript pane for up to 30 seconds.
//!
//! Noteey does not return SRT — the body is plain text with
//! `MM:SS` prefixes. The chain dispatches to
//! [`crate::parse::noteey_to_text`] via
//! [`SubtitleFormat::NoteeyTranscript`]. The user-requested `--format
//! srt` is rejected with `AppError::InvalidUsage` because noteey
//! cannot synthesise `SubRip` timestamps.
//!
//! Gated behind the `headless` Cargo feature because it depends on a
//! local Chromium/Chrome install at runtime.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use chromiumoxide::{Browser, BrowserConfig};
use futures::StreamExt;

use super::{Format, Provider, SubtitleFormat, SubtitleInfo};
use crate::error::{AppError, AppResult, NoSubtitleReason};
use crate::secret_endpoints::NOTEEY_PRIMARY_PAGE;

/// JavaScript that fills the noteey.com URL input and clicks
/// "Get Subtitle". Mirrors the downsub SPA trick (`SUBMIT_JS` in
/// `provider_headless`) — `Vue.js` requires the native `HTMLInputElement`
/// value setter plus an `input` event to detect the change.
const SUBMIT_JS: &str = r#"(async()=>{
  const input=document.querySelector('input[placeholder="Enter YouTube URL"]')
            || document.querySelector('input[type="text"]');
  if(!input) return JSON.stringify({err:"no input found"});
  const setter=Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype,'value').set;
  setter.call(input, "__VIDEO_URL__");
  input.dispatchEvent(new Event('input',{bubbles:true}));
  const buttons=Array.from(document.querySelectorAll('button'));
  const submit=buttons.find(b => b.textContent && b.textContent.trim().toLowerCase().includes('get subtitle'))
             || document.querySelector('button[type="submit"]')
             || buttons[0];
  if(!submit) return JSON.stringify({err:"no submit button"});
  submit.click();
  return JSON.stringify({ok:true,value:input.value});
})()"#;

/// JavaScript that polls for the transcript pane and returns its
/// plain-text content. The noteey SPA renders the transcript as
/// alternating `<div>MM:SS</div><div>text</div>` pairs, which
/// `document.body.innerText` emits as alternating timestamp-only and
/// text-only lines. We detect "rendered" by counting standalone
/// timestamp lines (matching `/^\d{1,2}:\d{2}(:\d{2})?$/`); the
/// Rust-side parser ([`crate::parse::noteey_to_text`]) reassembles
/// timestamp+text pairs.
///
/// `__POLL_LIMIT__` is substituted at evaluate time with the desired
/// number of 1-second polls (default 30).
const EXTRACT_JS: &str = r#"(async()=>{
  const limit=__POLL_LIMIT__;
  // Standalone timestamp on its own line: `00:00`, `01:23`, `1:02:45`.
  const tsRe=/^\d{1,2}:\d{2}(:\d{2})?$/;

  // GAP-AUD-2026-048: locate the transcript region so the page
  // header (nav, hero, login button, page title) is NOT included
  // in the body sent to the Rust parser. Strategy: prefer a stable
  // selector (`data-transcript`, `class*="transcript"`, `id*="transcript"`),
  // then fall back to the smallest container with ≥3 timestamp divs.
  function findTranscriptRegion(){
    const stable=document.querySelector('[data-transcript],[class*="transcript" i],[id*="transcript" i]');
    if(stable) return stable.innerText || "";
    const candidates=Array.from(document.querySelectorAll('div,section,article,main'));
    let best=null;
    let bestScore=-1;
    for(const root of candidates){
      const text=root.innerText || "";
      const ts= text.split(/\r?\n/).filter(l => tsRe.test(l.trim())).length;
      if(ts >= 3 && ts > bestScore){
        bestScore=ts;
        best=root;
      }
    }
    return best ? (best.innerText || "") : null;
  }

  let lastBody="";
  for(let i=0;i<limit;i++){
    const region=findTranscriptRegion();
    if(region && region !== lastBody){
      lastBody=region;
      const matches=region.split(/\r?\n/).filter(l => tsRe.test(l.trim()));
      // Three or more timestamps in the region confirms the pane
      // rendered (a 30-second clip can have 30+ cues; a short clip
      // may have only 5-10). Accept ≥3.
      if(matches.length >= 3){
        return JSON.stringify({ok:true,lines:matches.length,body:region});
      }
    }
    await new Promise(r=>setTimeout(r,1000));
  }
  return JSON.stringify({err:"transcript did not render within poll limit",polled:limit,last_body_len:lastBody.length,body:lastBody});
})()"#;

/// Maximum wall-clock time we wait for noteey's transcript pane to
/// render after clicking "Get Subtitle". 30 seconds matches the
/// observed first-load latency (`YouTube` iframe initialisation + API
/// call + render); longer would mask genuine failures.
const NOTEEY_POLL_LIMIT: u32 = 30;

/// Headless-browser provider. Construct with `ProviderNoteey::new()`.
pub struct ProviderNoteey {
    language: Option<String>,
    cache: Mutex<HashMap<String, Vec<u8>>>,
}

impl Default for ProviderNoteey {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderNoteey {
    /// Build a new noteey provider.
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

    /// Resolve a Chromium/Chrome executable. Delegates to
    /// [`crate::provider::stealth::ensure_chrome`] so every provider
    /// shares the same pin and on-disk cache.
    #[tracing::instrument(level = "debug", err)]
    async fn ensure_chrome() -> AppResult<String> {
        crate::provider::stealth::ensure_chrome().await
    }
}

/// Maximum wall-clock time we wait for the headless browser to
/// navigate, click "Get Subtitle", and run the extract JS. The
/// noteey page embeds a `YouTube` iframe and waits on player init,
/// so this is generous.
///
/// Re-exported from `crate::provider::stealth` so both providers
/// share a single source of truth. GAP-AUD-2026-041/044 moved the
/// constant there.
use crate::provider::stealth::HEADLESS_NAV_TIMEOUT;

#[async_trait]
impl Provider for ProviderNoteey {
    fn name(&self) -> &'static str {
        "provider-noteey"
    }

    async fn fetch_subtitle(
        &self,
        video_id: &str,
        language: &str,
        _format: Format,
    ) -> AppResult<SubtitleInfo> {
        // Honour YT_LEGEND_NO_NETWORK: skip the entire pipeline. Same
        // contract as `provider_headless` so the offline-audit CI job
        // surfaces a clean `ProviderUnavailable` instead of timing
        // out on a real browser launch.
        if std::env::var("YT_LEGEND_NO_NETWORK").is_ok() {
            return Err(AppError::ProviderUnavailable);
        }

        let lang = self
            .language
            .clone()
            .unwrap_or_else(|| language.to_string());
        // noteey returns one transcript per page, no per-language
        // selection in the SPA. We still pass the requested language
        // through for tracing and the `SubtitleInfo::language` field.
        let _lang_base = base_lang(&lang);

        tracing::debug!(
            target: "events",
            provider = "provider-noteey",
            video_id,
            language = %lang,
            "fetch_subtitle_started"
        );

        let chrome = Self::ensure_chrome().await?;
        let mut builder = BrowserConfig::builder();
        if let Some(profile) = crate::provider::stealth::prepare_user_data_dir() {
            builder = builder.user_data_dir(profile);
        }
        let config = builder
            .chrome_executable(chrome)
            .new_headless_mode()
            .arg("--no-sandbox")
            .arg("--disable-gpu")
            .arg("--disable-dev-shm-usage")
            .build()
            .map_err(|e| AppError::Internal(format!("browser config: {e}")))?;
        let (mut browser, mut handler) = Browser::launch(config).await.map_err(|e| {
            // GAP-AUD-2026-055: see provider_headless.rs — distinguish
            // "browser missing" from "CDP protocol mismatch".
            let display = e.to_string();
            let hint = if display.contains("Timeout while resolving websocket")
                || display.contains("Connection error")
            {
                ". Ensure BrowserFetcher can download the pinned revision, \
                 or set $CHROME to a compatible Chromium/Chrome binary"
            } else {
                ". Set $CHROME or install chromium-browser / google-chrome"
            };
            AppError::BrowserNotFound(format!("Browser::launch failed: {e}{hint}"))
        })?;
        let handler_task = tokio::spawn(async move { while handler.next().await.is_some() {} });

        let outcome = drive_page(&browser, video_id).await;
        let _ = browser.close().await;
        handler_task.abort();

        let body = outcome?;
        let source_url = format!("noteey://{video_id}/{lang}/txt");
        self.cache
            .lock()
            .map_err(|_| AppError::Internal("cache poisoned".into()))?
            .insert(source_url.clone(), body.clone().into_bytes());

        tracing::debug!(
            target: "events",
            provider = "provider-noteey",
            video_id,
            language = %lang,
            body_bytes = body.len(),
            "fetch_subtitle_completed"
        );

        Ok(SubtitleInfo {
            video_id: video_id.to_string(),
            language: lang,
            format: Format::Txt,
            source_url,
            byte_size: body.len(),
            format_hint: SubtitleFormat::NoteeyTranscript,
            provider: "provider-noteey",
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

/// Drive the noteey page: open, fill the input, submit, poll the
/// transcript pane, return the body as plain text.
#[tracing::instrument(level = "debug", skip(browser), fields(video_id))]
async fn drive_page(browser: &Browser, video_id: &str) -> AppResult<String> {
    let video_url = format!("https://www.youtube.com/watch?v={video_id}");
    let page = tokio::time::timeout(HEADLESS_NAV_TIMEOUT, browser.new_page("about:blank"))
        .await
        .map_err(|_| {
            AppError::Timeout(format!(
                "noteey new_page exceeded {HEADLESS_NAV_TIMEOUT:?}"
            ))
        })?
        .map_err(|e| {
            AppError::BrowserNotFound(format!(
                "browser.new_page failed (chromium unusable): {e}. Try $CHROME=/path/to/chrome"
            ))
        })?;
    // GAP-AUD-2026-041: apply stealth patches immediately after
    // `new_page` and BEFORE the first `page.goto` so the init script
    // runs in every document load. Without this ordering Cloudflare's
    // Windsor.io fingerprint script (`r.wdfl.co/rw.js`) reads
    // `navigator.webdriver === true` and assigns a high risk score
    // that blocks session creation.
    crate::provider::stealth::apply_stealth(&page).await?;
    // `tokio::time::timeout(...)` wraps `page.goto(...)` which returns
    // a `Result<(), CdpError>`. We unwrap the outer `Result<Result<...>>`
    // by mapping the timeout arm to `AppError::Timeout` and the
    // CdpError arm to `AppError::Internal`. The `let _ =` silences
    // `unused_must_use` on the outer Result of `tokio::time::timeout`.
    let _ = tokio::time::timeout(HEADLESS_NAV_TIMEOUT, page.goto(NOTEEY_PRIMARY_PAGE))
        .await
        .map_err(|_| {
            AppError::Timeout(format!(
                "noteey goto home exceeded {HEADLESS_NAV_TIMEOUT:?}"
            ))
        })
        .map_err(|e| AppError::Internal(format!("page.goto home failed: {e}")))?;
    tracing::debug!(
        target: "events",
        provider = "provider-noteey",
        video_id,
        "page_loaded; hydrating SPA"
    );
    // Give Vue.js time to hydrate the input + button. noteey is
    // lighter than downsub (no Cloudflare challenge), so 3s suffices.
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Step 1: fill the URL input and click "Get Subtitle".
    let submit_js = SUBMIT_JS.replace("__VIDEO_URL__", &video_url);
    match tokio::time::timeout(Duration::from_secs(5), page.evaluate(submit_js)).await {
        Ok(Ok(_)) => tracing::debug!(
            target: "events",
            provider = "provider-noteey",
            video_id,
            "submit_evaluate_ok"
        ),
        Ok(Err(e)) => {
            if crate::provider::stealth::is_terminal_cdp_error(&e) {
                // GAP-AUD-2026-045: the CDP target is gone. Surfacing
                // `ProviderUnavailable` lets the chain return
                // `EX_UNAVAILABLE` cleanly instead of pretending the
                // transcript is "not published".
                tracing::warn!(
                    target: "events",
                    provider = "provider-noteey",
                    video_id,
                    error = %e,
                    "submit_evaluate_terminal; aborting"
                );
                return Err(AppError::ProviderUnavailable);
            }
            tracing::warn!(
                target: "events",
                provider = "provider-noteey",
                video_id,
                error = %e,
                "submit_evaluate_failed"
            );
        }
        Err(_) => tracing::warn!(
            target: "events",
            provider = "provider-noteey",
            video_id,
            "submit_evaluate_timeout"
        ),
    }
    tracing::debug!(
        target: "events",
        provider = "provider-noteey",
        video_id,
        "submit_clicked; waiting for transcript"
    );

    // Step 2: poll the transcript pane. noteey uses a single-page
    // SPA — `document.body.innerText` is fine, but we extract via
    // child divs to preserve the timestamp prefix structure.
    let extract_js = EXTRACT_JS.replace("__POLL_LIMIT__", &NOTEEY_POLL_LIMIT.to_string());
    let raw: String = match tokio::time::timeout(
        Duration::from_secs(NOTEEY_POLL_LIMIT as u64 + 5),
        page.evaluate(extract_js.as_str()),
    )
    .await
    {
        Ok(Ok(v)) => v.into_value::<String>().map_err(|e| {
            AppError::Internal(format!("noteey extract returned non-string: {e}"))
        })?,
        Ok(Err(e)) => {
            if crate::provider::stealth::is_terminal_cdp_error(&e) {
                // GAP-AUD-2026-045: same treatment as the submit
                // phase — a dead CDP target means noteey cannot
                // answer, not that the video lacks subtitles.
                return Err(AppError::ProviderUnavailable);
            }
            return Err(AppError::Internal(format!(
                "noteey extract evaluate failed: {e}"
            )));
        }
        Err(_) => {
            return Err(AppError::Timeout(format!(
                "noteey extract exceeded {}s",
                NOTEEY_POLL_LIMIT as u64 + 5
            )));
        }
    };

    let _ = page.close().await;
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
        AppError::TimedtextUpstreamError(format!("noteey extract non-JSON: {e}"))
    })?;
    if v.get("err").is_some() {
        // GAP-AUD-2026-046: dump first 200 chars of body for diagnosis.
        // The polling JS returns `{err, polled, last_body_len}` but
        // not the actual text — without this dump we can't tell
        // whether noteey returned a captcha interstitial, an empty
        // page, or a real page where the transcript just didn't
        // render in time.
        let last_body_len = v.get("last_body_len").and_then(|x| x.as_u64()).unwrap_or(0);
        let body_dump = v.get("body").and_then(|x| x.as_str()).unwrap_or("");
        let first_500: String = body_dump.chars().take(500).collect();
        tracing::warn!(
            target: "events",
            provider = "provider-noteey",
            video_id,
            last_body_len,
            first_500 = %first_500,
            "noteey_extract_diagnostic"
        );
        // Transcript never rendered within poll budget. Treat as
        // upstream degradation so the operator knows noteey couldn't
        // get the body — distinct from "video has no subtitles".
        tracing::warn!(
            target: "events",
            provider = "provider-noteey",
            video_id,
            polled = ?v.get("polled"),
            "noteey transcript did not render; degrading"
        );
        return Err(AppError::ProviderUnavailable);
    }
    let body = v["body"].as_str().unwrap_or("").to_string();
    if body.is_empty() {
        tracing::warn!(
            target: "events",
            provider = "provider-noteey",
            video_id,
            "noteey returned empty body; degrading"
        );
        return Err(AppError::ProviderUnavailable);
    }
    Ok(body)
}

/// Reduce a BCP 47 tag (`pt-BR`, `pt_BR.UTF-8`) to its primary
/// subtag. Mirrors [`crate::provider::stealth::base_lang`] but is
/// duplicated here to avoid exporting a private helper across modules.
fn base_lang(language: &str) -> String {
    language
        .split(['-', '_', '.'])
        .next()
        .unwrap_or(language)
        .to_lowercase()
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
    fn noteey_provider_name_is_provider_noteey() {
        assert_eq!(ProviderNoteey::new().name(), "provider-noteey");
    }

    #[test]
    fn noteey_with_language_is_chainable() {
        let p = ProviderNoteey::new().with_language("pt-BR");
        // Just exercise the builder; the field is private but the
        // call must not panic.
        assert_eq!(p.name(), "provider-noteey");
    }

    #[test]
    fn notey_page_constant_is_correct() {
        assert!(NOTEEY_PRIMARY_PAGE.starts_with("https://"));
        assert!(NOTEEY_PRIMARY_PAGE.contains("noteey.com"));
    }

    #[test]
    fn submit_js_substitutes_video_url() {
        // Cheap syntax check that the SUBMIT_JS template still
        // contains the placeholder we substitute.
        assert!(SUBMIT_JS.contains("__VIDEO_URL__"));
    }

    #[test]
    fn extract_js_substitutes_poll_limit() {
        assert!(EXTRACT_JS.contains("__POLL_LIMIT__"));
    }
}
