//! Anti-fingerprint patches for chromiumoxide providers.
//!
//! Closes GAP-AUD-2026-041 and GAP-AUD-2026-044.
//!
//! Both `provider_headless` (downsub.com) and `provider_noteey`
//! (noteey.com) launch a Chromium instance via `chromiumoxide`. In
//! `--headless=new` mode Chromium exposes a handful of trivially
//! detectable signals that bump the Cloudflare Turnstile / Windsor.io
//! risk score high enough to block session creation in datacenter
//! IPs.
//!
//! The patches below are injected via the CDP
//! `Page.addScriptToEvaluateOnNewDocument` call (exposed in
//! chromiumoxide 0.9.1 as [`Page::evaluate_on_new_document`]). The
//! script runs in every new document before any other JS, so the
//! `navigator.webdriver` getter is masked before Windsor.io reads
//! it.
//!
//! # What the script masks
//!
//! 1. `navigator.webdriver` — set to `undefined` (real Chrome returns
//!    `false` for a navigator that has never heard of `webdriver`,
//!    but `Object.defineProperty` with an undefined getter is the
//!    convention used by `puppeteer-extra` / `playwright-extra` and
//!    passes the open-source fingerprintjs.com probes).
//! 2. `navigator.plugins` — headless Chromium ships with zero
//!    plugins. We inject a 3-entry array mimicking the default
//!    Chromium load (`Chrome PDF Plugin`, `Chrome PDF Viewer`,
//!    `Native Client`).
//! 3. `navigator.languages` — headless reports `["en-US"]`; real
//!    users on Brazilian machines report `["pt-BR", "en-US", "en"]`.
//! 4. `WebGLRenderingContext.prototype.getParameter` for constants
//!    `37445` (`UNMASKED_VENDOR_WEBGL`) and `37446`
//!    (`UNMASKED_RENDERER_WEBGL`). Headless reports
//!    `"Google Inc. (SwiftShader)"` because the `SwiftShader` software
//!    rasterizer kicks in; we override with `Intel Inc.` /
//!    `Intel Iris OpenGL Engine`.
//! 5. `window.chrome.runtime` — headless omits the `chrome.runtime`
//!    surface; we install a minimal mock object so any `chrome?.`
//!    probing code does not throw.
//!
//! # Companion Chromium flag
//!
//! `--disable-blink-features=AutomationControlled` (set in the
//! `BrowserConfig` of both providers) instructs Blink at the
//! process level to skip the `navigator.webdriver = true` injection
//! that the `--enable-automation` flag would otherwise apply. The
//! CDP patch above is the *defence in depth* for sites that probe
//! later or that the Blink flag does not reach.

use chromiumoxide::fetcher::{BrowserVersion, Revision};
use chromiumoxide::Page;
use std::time::Duration;

use crate::error::{AppError, AppResult};

/// Default navigation timeout shared by all headless providers.
/// Pulled out of `provider_headless.rs` and `provider_noteey.rs` to
/// avoid silent drift between the two.
pub(crate) const HEADLESS_NAV_TIMEOUT: Duration = Duration::from_secs(60);

/// Chromium revision pinned as compatible with `chromiumoxide` 0.9.1.
///
/// Resolution: `r1585606` matches the version that `chromiumoxide` 0.9.0
/// bumped in its fetcher (per the crate CHANGELOG). The earlier pin
/// (`r1378488`) targeted a Chromium series that has since been removed
/// from `chromium-browser-snapshots`, and `BrowserFetcher` fails to
/// fall back gracefully: the pin must match a real object in GCS.
pub const COMPATIBLE_CHROMIUM_REVISION: u32 = 1585606;

/// Classify a chromiumoxide CDP error as terminal (no point retrying) or
/// transient (worth retrying). Returns `true` when the error message
/// indicates the CDP target is gone — common cases are `Error -32000:
/// Inspected target navigated or closed`, websocket resets, and
/// `NoResponse`. Retrying these burns 80+ s of wall-clock time before
/// the timeout fires (see GAP-AUD-2026-045).
///
/// We match on the `Display` string because `chromiumoxide_types::Error`
/// variants like `JsSendException` are re-exported under different
/// paths across patch versions; string matching against the canonical
/// Chrome `DevTools` Protocol error messages is version-stable.
pub(crate) fn is_terminal_cdp_error(e: &chromiumoxide::error::CdpError) -> bool {
    let msg = e.to_string();
    msg.contains("Inspected target navigated or closed")
        || msg.contains("No response received")
        || msg.contains("websocket")
        || msg.contains("Target closed")
        || msg.contains("Session closed")
        || msg.contains("NoResponse")
}

/// Base language code: `pt-BR` and `pt_BR.UTF-8` both reduce to `pt`.
///
/// Shared helper kept for every chromiumoxide provider. `provider_noteey`
/// carries its own copy for tracing-only use, so this canonical version
/// is currently exercised by its unit test rather than a call site.
#[allow(dead_code)]
pub(crate) fn base_lang(language: &str) -> String {
    language
        .split(['-', '_', '.'])
        .next()
        .unwrap_or(language)
        .to_lowercase()
}

/// Async-only: resolve a Chromium/Chrome executable path. Order:
/// 1. `$CHROME` (operator override).
/// 2. `BrowserFetcher` auto-download at the pinned revision. This
///    is preferred over the system browser because system Chromium
///    builds (e.g. Fedora 44 ships Chromium 149) are frequently
///    newer than the CDP protocol `chromiumoxide` 0.9.1 was built
///    against, which makes `Browser::launch` fail with `WS
///    Connection error: Protocol(ResetWithoutClosingHandshake)`.
/// 3. Well-known system paths (`/usr/bin/chromium-browser`, etc.) as
///    a last-resort fallback for operators who intentionally want
///    the system browser.
///
/// GAP-AUD-2026-038: shared by every chromiumoxide provider so they
/// reuse the same Chromium pin and on-disk cache, avoiding a double
/// download.
///
/// # Errors
///
/// - [`AppError::BrowserNotFound`] when the cache directory cannot
///   be created, `BrowserFetcher` fails to download the pinned
///   revision, and no system browser exists on `$CHROME` or any
///   of the well-known paths.
/// - [`AppError::Internal`] when `BrowserFetcherOptions` cannot
///   be built.
#[tracing::instrument(level = "debug", err)]
pub async fn ensure_chrome() -> AppResult<String> {
    if let Ok(p) = std::env::var("CHROME") {
        if !p.is_empty() {
            return Ok(p);
        }
    }
    let cache_dir =
        directories::ProjectDirs::from("com", "youtube-legend-cli", "youtube-legend-cli")
            .map(|p| p.cache_dir().join("browser"))
            .unwrap_or_else(|| std::env::temp_dir().join("yt-legend-browser"));
    if let Err(e) = tokio::fs::create_dir_all(&cache_dir).await {
        return Err(AppError::BrowserNotFound(format!(
            "cannot create cache dir {}: {e}",
            cache_dir.display()
        )));
    }
    let fetcher = chromiumoxide::fetcher::BrowserFetcher::new(
        chromiumoxide::fetcher::BrowserFetcherOptions::builder()
            .with_path(&cache_dir)
            .with_version(BrowserVersion::Revision(Revision::new(
                COMPATIBLE_CHROMIUM_REVISION,
            )))
            .build()
            .map_err(|e| AppError::Internal(format!("BrowserFetcherOptions: {e}")))?,
    );
    match fetcher.fetch().await {
        Ok(info) => {
            tracing::info!(
                target: "events",
                path = %info.executable_path.display(),
                "BrowserFetcher resolved executable"
            );
            return info
                .executable_path
                .to_str()
                .map(str::to_string)
                .ok_or_else(|| {
                    AppError::BrowserNotFound(format!(
                        "BrowserFetcher returned non-utf8 path: {}",
                        info.executable_path.display()
                    ))
                });
        }
        Err(e) => {
            tracing::warn!("BrowserFetcher download failed ({e}); falling back to system chromium");
        }
    }
    let sys = chrome_path().ok_or_else(|| {
        AppError::BrowserNotFound(
            "no chromium found: BrowserFetcher download failed and no system browser in \
             $CHROME or well-known paths. Ensure BrowserFetcher can download the pinned \
             revision, or set $CHROME to a compatible Chromium/Chrome binary"
                .to_string(),
        )
    })?;
    tracing::warn!(
        target: "events",
        "using system browser at {sys} — it may be incompatible with \
         chromiumoxide 0.9.1's CDP protocol; set $CHROME to the \
         BrowserFetcher-pinned revision for reliable operation"
    );
    Ok(sys)
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

/// JavaScript source applied to every new document via
/// [`Page::evaluate_on_new_document`]. The patches are idempotent
/// — calling `apply_stealth` on a page that already has the patches
/// installed is a no-op (every `Object.defineProperty` call uses
/// `configurable: true` by default and overrides the previous
/// value).
///
/// Source-string form is preferred over `include_str!` so the
/// content shows up in `cargo doc --document-private-items` and in
/// the rust-analyzer hover view.
pub(crate) const STEALTH_INIT_JS: &str = r#"
// Patch 1: navigator.webdriver undefined
Object.defineProperty(navigator, 'webdriver', { get: () => undefined });

// Patch 2: navigator.plugins with 3 real-looking entries
Object.defineProperty(navigator, 'plugins', {
  get: () => [
    { name: 'Chrome PDF Plugin', filename: 'internal-pdf-viewer', length: 1 },
    { name: 'Chrome PDF Viewer', filename: 'mhjfbmdgcfjbbpaeojofohoefgiehjai', length: 1 },
    { name: 'Native Client', filename: 'internal-nacl-plugin', length: 2 }
  ]
});

// Patch 3: navigator.languages with 3 entries
Object.defineProperty(navigator, 'languages', {
  get: () => ['pt-BR', 'en-US', 'en']
});

// Patch 4: WebGL vendor override (masks SwiftShader)
const origGetParameter = WebGLRenderingContext.prototype.getParameter;
WebGLRenderingContext.prototype.getParameter = function(param) {
  if (param === 37445) return 'Intel Inc.';
  if (param === 37446) return 'Intel Iris OpenGL Engine';
  return origGetParameter.call(this, param);
};

// Patch 5: chrome.runtime mock
window.chrome = { runtime: {}, csi: () => {}, loadTimes: () => {} };
"#;

/// Apply all stealth patches to the page before navigation.
///
/// Must be called **after** `browser.new_page(...)` and **before**
/// `page.goto(...)` so the init script runs in the document
/// bootstrap. Returns the `ScriptIdentifier` returned by CDP so
/// callers can later remove the script via
/// `Page.removeScriptToEvaluateOnNewDocument` (currently unused —
///// exposed for future fingerprint rotation per session).
///
/// # Errors
///
/// Returns [`AppError::Internal`] when the CDP call itself fails
/// (e.g. browser was killed between `new_page` and the patch).
#[tracing::instrument(level = "debug", skip(page), fields(provider = tracing::field::Empty))]
pub async fn apply_stealth(page: &Page) -> crate::error::AppResult<()> {
    page.evaluate_on_new_document(STEALTH_INIT_JS)
        .await
        .map(|_script_id| ())
        .map_err(|e| crate::error::AppError::Internal(format!("apply_stealth failed: {e}")))
}

/// Resolve a Chromium profile directory isolated under the project
/// cache, removing any stale Chrome singleton lock files left behind by
/// prior crashed runs.
///
/// GAP-AUD-2026-055: chromiumoxide 0.9.1 defaults `user_data_dir` to
/// `/tmp/chromiumoxide-runner/`, which is shared across all chromium
/// invocations on the host. When a previous launch crashed before
/// `browser.close()`, the leftover `SingletonLock`, `SingletonCookie`,
/// and `SingletonSocket` symlinks point at PIDs that no longer exist,
/// and the next `Browser::launch` aborts with
/// `Failed to create /tmp/chromiumoxide-runner/SingletonLock: Arquivo
/// existe (17)` and `exit 5376`. The chromiumoxide error message is
/// generic ("`Browser::launch` failed") and gives no hint about the
/// actual cause.
///
/// The fix is twofold:
/// 1. Anchor `user_data_dir` under the project's XDG cache
///    (`~/.cache/youtube-legend-cli/chrome-profile/`) so two providers
///    cannot race on the same profile, and so operator-side cleanup
///    (`rm -rf ~/.cache/youtube-legend-cli`) covers every stale lock
///    the CLI has ever produced.
/// 2. Sweep any orphan singleton symlinks before each launch. Chrome's
///    own `ProcessSingleton` re-creates them on demand; if a symlink
///    points at a dead PID we delete it instead of letting Chrome
///    abort.
///
/// Returns the path to feed into [`chromiumoxide::BrowserConfig::user_data_dir`].
#[tracing::instrument(level = "debug")]
pub fn prepare_user_data_dir() -> Option<std::path::PathBuf> {
    let base = directories::ProjectDirs::from("com", "youtube-legend-cli", "youtube-legend-cli")
        .map(|p| p.cache_dir().join("chrome-profile"))
        .unwrap_or_else(|| std::env::temp_dir().join("yt-legend-chrome-profile"));
    if std::fs::create_dir_all(&base).is_err() {
        return None;
    }
    // Sweep orphan singleton files. The symlink targets encode the PID
    // that owned the lock at creation time; if the PID is no longer
    // alive, the symlink is stale and Chrome refuses to start.
    for name in ["SingletonLock", "SingletonCookie", "SingletonSocket"] {
        let path = base.join(name);
        if let Ok(meta) = std::fs::symlink_metadata(&path) {
            if meta.file_type().is_symlink() {
                if let Ok(target) = std::fs::read_link(&path) {
                    if !is_pid_alive_from_symlink(&target) {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            } else {
                // Plain file with stale mtime (>1h) is also suspect.
                if let Ok(modified) = meta.modified() {
                    if modified.elapsed().unwrap_or_default() > std::time::Duration::from_secs(3600)
                    {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
    }
    Some(base)
}

/// Best-effort PID liveness check for the target of a stale
/// `SingletonLock`/`SingletonCookie` symlink. The chromiumoxide path
/// embeds either a hostname token (e.g. `fedora-3016353`) or a numeric
/// PID (e.g. `/tmp/org.chromium.Chromium.abc/SingletonSocket`).
/// Numeric targets can be probed via `kill -0`; non-numeric tokens
/// (where Chrome stored the lock under a hostname alias) are treated
/// as stale because they always outlive a single `Browser::launch`.
#[cfg(unix)]
fn is_pid_alive_from_symlink(target: &std::path::Path) -> bool {
    let Some(file_name) = target.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let digits: String = file_name
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let Ok(pid) = digits.parse::<i32>() else {
        return false;
    };
    if pid <= 0 {
        return false;
    }
    // `kill -0 <pid>` returns 0 if the PID exists and we may signal
    // it; non-zero otherwise. This is the POSIX way to probe liveness
    // without actually delivering a signal.
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_pid_alive_from_symlink(_target: &std::path::Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GAP-AUD-2026-044 contract: every anti-fingerprint signal must
    /// be present in the init script. If a regression drops one of
    /// the patches (typo, accidental edit), this assertion fires
    /// loudly during `cargo test`.
    #[test]
    fn stealth_init_js_masks_webdriver() {
        assert!(
            STEALTH_INIT_JS.contains("navigator, 'webdriver'"),
            "Patch 1 (navigator.webdriver) is missing — Cloudflare Turnstile \
             will detect headless Chromium."
        );
    }

    #[test]
    fn stealth_init_js_pollutes_plugins() {
        assert!(
            STEALTH_INIT_JS.contains("navigator, 'plugins'"),
            "Patch 2 (navigator.plugins) is missing — fingerprint will report \
             zero plugins (distinctive headless signature)."
        );
        assert!(
            STEALTH_INIT_JS.contains("Chrome PDF Plugin"),
            "Plugin list is empty — the three default Chromium plugins must be listed."
        );
    }

    #[test]
    fn stealth_init_js_overrides_languages() {
        assert!(
            STEALTH_INIT_JS.contains("navigator, 'languages'"),
            "Patch 3 (navigator.languages) is missing — fingerprint will report \
             ['en-US'] (distinctive headless signature)."
        );
        assert!(
            STEALTH_INIT_JS.contains("pt-BR"),
            "languages must include pt-BR to match Brazilian operator locale."
        );
    }

    #[test]
    fn stealth_init_js_overrides_webgl_vendor() {
        assert!(
            STEALTH_INIT_JS.contains("37445"),
            "Patch 4 (WebGL UNMASKED_VENDOR_WEBGL constant) is missing — \
             fingerprint will report 'Google Inc. (SwiftShader)'."
        );
        assert!(
            STEALTH_INIT_JS.contains("37446"),
            "Patch 4 (WebGL UNMASKED_RENDERER_WEBGL constant) is missing — \
             fingerprint will report the SwiftShader renderer string."
        );
        assert!(
            STEALTH_INIT_JS.contains("Intel Inc."),
            "WebGL vendor override target is wrong — must impersonate Intel."
        );
    }

    #[test]
    fn stealth_init_js_mocks_chrome_runtime() {
        assert!(
            STEALTH_INIT_JS.contains("chrome.runtime"),
            "Patch 5 (window.chrome.runtime mock) is missing — fingerprint will \
             see `window.chrome === undefined`."
        );
        assert!(
            STEALTH_INIT_JS.contains("runtime: {}"),
            "chrome.runtime mock must be an empty object, not undefined."
        );
    }

    #[test]
    fn headless_nav_timeout_is_60s() {
        // Both providers rely on this exact value — any drift breaks
        // the SLO documented in CHANGELOG.md (GAP-AUD-2026-037).
        assert_eq!(HEADLESS_NAV_TIMEOUT, Duration::from_secs(60));
    }

    #[test]
    fn base_lang_reduces_region_tags() {
        assert_eq!(base_lang("pt-BR"), "pt");
        assert_eq!(base_lang("pt_BR.UTF-8"), "pt");
        assert_eq!(base_lang("EN"), "en");
        assert_eq!(base_lang("es"), "es");
    }

    #[test]
    fn compatible_chromium_revision_is_pinned() {
        // GAP-AUD-003 fix: revision must be (a) non-zero, (b) post the
        // CDP protocol events that `chromiumoxide` 0.9.1 expects,
        // and (c) available in `chromium-browser-snapshots` on GCS.
        const { assert!(COMPATIBLE_CHROMIUM_REVISION > 0) };
        const { assert!(COMPATIBLE_CHROMIUM_REVISION > 1_000_000) };
    }

    #[test]
    fn browser_version_constructs_from_pinned_revision() {
        let version = BrowserVersion::Revision(Revision::new(COMPATIBLE_CHROMIUM_REVISION));
        let display = format!("{version:?}");
        assert!(
            display.contains("Revision"),
            "expected Revision variant in {display}"
        );
        assert!(
            display.contains(&COMPATIBLE_CHROMIUM_REVISION.to_string()),
            "expected pinned revision in {display}"
        );
    }
}
