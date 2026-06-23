# Changelog
[English](CHANGELOG.md) | [Português Brasileiro](CHANGELOG.pt-BR.md)

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.3.3] - 2026-06-23

### Fixed
- **GAP-AUD-2026-060**: JSON error envelope now emitted to stdout for pre-fetch validation errors when `--json` is active; previously stdout was empty
- **GAP-AUD-2026-061**: added `language_detected: false` field to the JSON success envelope signaling that the language reflects the requested locale, not a detected one
- **GAP-AUD-2026-062**: `>>` speaker-change markers from interview transcripts now stripped by `noteey_to_text` parser
- **GAP-AUD-2026-063**: documentation in `docs/AGENTS.md` and `docs/AGENTS.pt-BR.md` updated to use `.content` (was `.body`) matching the actual JSON envelope
- **GAP-AUD-2026-064**: duplicate stderr line for invalid URL errors eliminated (side effect of GAP-060 fix)
- **GAP-AUD-2026-065**: `byte_size` in JSON envelope now reflects the cleaned NFC content length, not the raw HTML body size
- **GAP-AUD-2026-066**: `--verbose` flag now works; previously it was a dead flag with no effect on logging output
- **GAP-AUD-2026-067**: stderr kill-signal noise from Chromium cleanup eliminated via `std::mem::forget(browser)` pattern
- **GAP-AUD-2026-068**: `--format srt` limitation with `provider-noteey` now documented in the `--help` text
- **GAP-AUD-2026-069**: batch `--json` output now emits proper NDJSON (newline-terminated); previously concatenated envelopes as `}{` breaking parsers like `jq`

### Changed
- Documentation audit: `llms.txt`, `llms-full.txt`, `COOKBOOK.md`, `INTEGRATIONS.md` and their PT-BR counterparts updated to reflect the v0.3.2 provider consolidation (removed stale references to `ProviderA`, `ProviderB`, `ProviderHeadless`, `youtube-direct`, `--asr`, `--no-fallback`)

## [0.3.2] - 2026-06-21

### Breaking
- REMOVED providers: `youtube-direct`, `provider-a`, `provider-b`, `provider-headless`. Only `provider-noteey` remains
- REMOVED flags: `--asr`, `--no-fallback`, `--headless`. These flags now produce exit 2 (clap rejection)
- `--provider` now accepts only `auto` and `provider-noteey`
- `provider-noteey` uses headless Chromium via `chromiumoxide 0.9.1` exclusively

### Added
- `provider-noteey` as the exclusive provider via headless Chromium (`chromiumoxide 0.9.1`)
- `BrowserFetcher` auto-downloads pinned Chromium r1585606 (version 147.0.7693.0) to `~/.cache/youtube-legend-cli/browser/`
- `.new_headless_mode()` for Chromium 147+ compatibility
- `prepare_user_data_dir()` in `stealth.rs` sweeps stale `Singleton{Lock,Cookie,Socket}` before browser launch
- `noteey_to_text` parser in `src/parse/mod.rs` strips `MM:SS`/`HH:MM:SS` timestamps from noteey transcripts
- `findTranscriptRegion` JS function isolates the transcript pane from page header/nav
- 11 regression tests for `noteey_to_text` covering timestamps, markers, Unicode NFC, 50 MiB cap

### Changed
- Architecture simplified to single provider `provider-noteey`
- `auto` chain now contains only `provider-noteey`
- `ensure_chrome()` logs resolved executable path via `tracing::info`

### Fixed (historical — applied during multi-provider era before provider removal)
- **GAP-AUD-003**: `chromiumoxide` warn-level events (the `WS Invalid message: data did not match any variant of untagged enum Message` loop) are silenced via `EnvFilter::add_directive("chromiumoxide=error")` in `src/logging.rs`. The handler logic already drops the unknown CDP message but the log line fired unconditionally. Operators can override with `YT_LOG_LEVEL=chromiumoxide=warn` for deeper investigation
- **GAP-E2E-001**: 4-layer log duplication per failed extraction collapsed to 1. `info! "fetch_subtitle_started|completed"` callsites in `provider_headless.rs:244,279`, `provider_a.rs:162,178`, `provider_b.rs:318,332`, `provider_youtube_direct.rs:153,160` downgraded to `debug!`. The chain warn at `provider/mod.rs:263` and the canonical error at `extract.rs:84` remain — they are the signal; the provider-level events were noise
- **GAP-E2E-009**: `--dry-run` now returns `ExitCode::SUCCESS` (0) and emits a stable `dry_run_cache_miss` JSON envelope to stdout instead of constructing `AppError::NoSubtitle(NotPublished)` and exiting with 66 (EX_NOINPUT). CI scripts can branch on the `event` field. New helper `output_dry_run` in `src/commands/mod.rs`
- **GAP-E2E-013**: Duplicated `config error:` prefix removed. `main.rs:27` now calls `eprintln!("{e}")` and lets `AppError::Config`'s Display (which already includes the prefix) handle the message. Operators see `config error: <path>` exactly once
- **GAP-E2E-014**: Retry warn logs at `retry.rs:52,63` downgraded to `debug!`. Three retries × two warn lines no longer pollute stderr under `--log-level info`
- **GAP-E2E-015**: `Cli::validate()` now returns `AppResult<()>` instead of `Result<(), String>`. The `String → AppError::InvalidUsage` bridge at `commands/mod.rs:42` is gone. 12 existing tests updated to assert `matches!(err, AppError::InvalidUsage(_))`
- **GAP-E2E-016**: Sentinel-based `apply_config_overrides` (which compared parsed fields against literal defaults like `if self.timeout == 30`) replaced by `CliOverrideFlags` bitmask populated via `ArgMatches::value_source` in the new `parse_with_overrides()` entry point. The previous logic silently mis-applied config overrides when the operator typed a flag explicitly with the same value as the built-in default; the new bitmask reports per-flag "was set on the command line?" deterministically. 3 existing tests updated and a new regression test added (`apply_config_overrides_explicit_default_does_not_get_overridden`)
- **GAP-E2E-017**: `parse_video_id_from_url` (in `commands/mod.rs`) routed through `tracing::info!` instead of direct `io::write_to_stderr`. The `--quiet` flag now actually silences the verbose line because the `tracing-subscriber` EnvFilter built in `logging.rs` intercepts it
- **GAP-E2E-018**: `player_js_cache.rs:136,158` warn logs downgraded to `debug!`. Cache miss race conditions no longer pollute stderr under `--log-level info`
- **GAP-E2E-024**: `extract_video_object_url` in `src/provider/provider_a.rs` refactored to use a depth-capped BFS with `Vec::with_capacity(8)` pre-allocation. The previous queue-based walker pushed every nested JSON value individually and had no defence against cyclic references. New helpers `walk_video_objects` (recursive with `MAX_DEPTH = 32`) and `video_object_caption` (single-object extractor) replace the monolithic function. 3 new regression tests cover shallow videoobject resolution, deep nesting truncation, and 1000-node wide documents within a 100ms budget
- **GAP-E2E-025**: `provider_b.rs:251-253` now returns `AppError::NoSubtitle(NoSubtitleReason::NotPublished)` (exit 66) when `sid`/`hash`/`hl` are empty, instead of `AppError::ProviderUnavailable` (exit 69). Empty session tokens mean the upstream did not generate them for this video — semantically identical to "no captions published". The previous behaviour trapped CI scripts in infinite retry loops because exit 69 suggests a transient failure
- **GAP-E2E-026**: HTTP 400 from upstream providers now maps to `NoSubtitle(NotPublished)` (exit 66) across all providers via `NoSubtitleReason::from_status(400)`. The previous mapping was inconsistent: `provider_a.rs:118-122` already returned `NoSubtitle` for 400, but the other 3 sites (`provider_a.rs:86-90`, `provider_b.rs:87-91`, `provider_b.rs:132-136`) returned `ProviderUnavailable` (exit 69). The unification treats 400 as "no captions exist" per the YouTube timedtext endpoint convention. **BREAKING** for callers that branched on exit 69 for 400 responses — they should now use `NoSubtitleReason::from_status(400) == Some(NotPublished)`. 2 integration tests updated
- **GAP-E2E-027**: `src/provider/robots.rs:73` no longer silently swallows `Ok(non-success)` responses. 5xx (transient) emits `tracing::warn!`, 4xx (definitive) emits `tracing::debug!`. Operators can now distinguish "robots.txt returned 503 (upstream problem, behaviour may change)" from "robots.txt returned 404 (does not exist, behaviour is definitive)". The fail-open semantics are preserved in both cases. 2 contract tests pin the log-level policy
- **GAP-E2E-028**: `ProviderYouTubeDirect::fetch_subtitle` now consults `robots.txt` before any request via `super::robots::check_allowed(YOUTUBE_HOST, "/watch", USER_AGENT_IDENTITY).await?;` matching the behaviour of `ProviderA` (line 161) and `ProviderB` (line 317). NFR-007 compliance is now uniform across all 3 providers. New constant `pub(crate) const YOUTUBE_HOST: &str = "www.youtube.com"` in `src/secret_endpoints.rs`. New integration test suite `tests/integration/provider_youtube_direct_wiremock.rs` (5 tests) covers the robots-txt match logic via wiremock
- **GAP-E2E-029**: `provider_youtube_direct.rs:153` debug event now uses `target: "events"` (consistent with the other 6 debug callsites in providers A/B/headless) instead of the orphan `target: "youtube_decipher"`. Operators with a dashboard filter on `target = "events"` now capture the n-parameter detection signal consistently
- **GAP-E2E-030**: `provider_b.rs:140-148` now returns the new `AppError::CaptchaChallenge { provider, kind }` variant when the response body contains `cf-turnstile` or `h-captcha`, instead of `AppError::ProviderUnavailable`. The new variant preserves exit 69 (backward-compatible with existing scripts) but allows programmatic distinction via the `AppError::is_captcha()` helper. Display includes the provider name and captcha implementation. 3 tests cover the new variant and the helper
- **GAP-E2E-031**: `provider_youtube_direct.rs:321-329` now returns `AppError::TimedtextUpstreamError` (exit 70) for unexpected `Content-Type` instead of `AppError::InvalidInput` (exit 64). The content-type comes from the YouTube upstream, not from operator input. The previous classification made the operator think the CLI was misused when the actual cause was upstream
- **GAP-E2E-032**: 6 sites in `src/parse/srv3.rs` (lines 78, 93, 96, 111, 137, 147, 196) now return `AppError::TimedtextUpstreamError` (exit 70) for parse failures of the YouTube payload instead of `AppError::InvalidInput` (exit 64). The body is upstream-originated, not operator input. The previous classification conflated parse errors of YouTube Srv3/JSON3 with operator CLI mistakes. 6 new tests cover the reclassification for `srv3_to_srt` (empty body, no `<text>` cues, invalid `start`, invalid `dur`) and `json3_to_srt` (empty body, no `events[]` array, no usable events). The existing `rejects_empty_body` test was updated to match the new variant
- **GAP-AUD-2026-033** (auditoria e2e de 2026-06-19): The `headless` Cargo feature is now enabled by default (`Cargo.toml:79 default = ["headless"]`). The previous default of `[]` meant the `provider-headless` path was unreachable in default builds, even though it was the only viable path against the YouTube anti-bot for datacenter IPs and against the CORS-restricted downsub.com endpoints. The previous behaviour trapped operators who installed via `cargo install youtube-legend-cli` and never discovered they needed `--features headless`. The fix preserves the escape hatch via `--no-default-features` for environments without a Chromium/Chrome runtime
- **GAP-AUD-2026-034** (auditoria e2e de 2026-06-19): `provider_headless.rs:24` now imports `PROVIDER_A_PRIMARY_PAGE` (downsub.com) instead of `PROVIDER_B_PRIMARY_PAGE` (downloadyoutubesubtitles.com). downsub.com is the site that operators actually use in the browser, has a Vue.js SPA with predictable DOM structure, and accepts the URL via the `?url=` query parameter (when combined with the input-and-submit interaction added by GAP-AUD-2026-036)
- **GAP-AUD-2026-035** (auditoria e2e de 2026-06-19): `DOWNLOAD_JS` in `src/provider/provider_headless.rs` rewritten to match the downsub.com DOM. The previous selector `document.querySelectorAll("a")` filtered by `e.dataset.href && /get2\.php/.test(e.dataset.href)` is the legacy downloadyoutubesubtitles.com contract; downsub.com renders `<button data-title="[TXT] Portuguese (auto-generated)">` wrapped in an `<a href="...">` anchor. The new selector `document.querySelectorAll("button, a")` filters by `e.dataset.title` containing a bracketed format tag, then climbs to the parent anchor to extract the real download URL. Polling budget raised to 45s × 1s (matches the user-reported "may take up to 30s" extraction time)
- **GAP-AUD-2026-036** (auditoria e2e de 2026-06-19): `drive_page` in `src/provider/provider_headless.rs` now does input-then-submit instead of bare `goto` to a `?url=` URL. downsub.com's Vue.js SPA does NOT auto-process the `?url=` query param — it only seeds the input field. The user (and now the CLI) must set the input value via the native `HTMLInputElement.prototype` setter, dispatch an `input` event so Vue.js detects the change, and click the submit button. New constant `SUBMIT_JS` holds the setter+dispatch+click sequence. The drive_page flow is now: open `about:blank` page → `goto(PROVIDER_A_PRIMARY_PAGE)` → sleep 5s for SPA hydration → `page.evaluate(SUBMIT_JS)` → sleep 5s for the click handler to trigger the extraction
- **GAP-AUD-2026-037** (auditoria e2e de 2026-06-19): `drive_page` now loops up to 20 times over `browser.pages()`, picking the first non-home page whose URL contains `downsub.com` and re-running `DOWNLOAD_JS` against it. The previous single-evaluate path always failed with `CdpError::ChannelClosed "Error -32000: Inspected target navigated or closed"` because the downsub submit triggers a hard navigation that closes the original CDP `Page` handle. The retry loop re-resolves the live target after each `wait_for_navigation` boundary, tolerating the SPA's navigation behaviour. 3s sleep between attempts gives the SPA enough time to populate the per-language download buttons

### Added (noteey fallback)
- **`provider-noteey`** added as the automatic fallback when downsub.com is degraded (GAP-AUD-2026-038). When `provider-headless` returns `ProviderUnavailable` (site unreachable, buttons absent after 45 polls, button-without-href, or non-200 fetch), the chain now tries noteey.com via a second headless provider. Both providers share the `BrowserFetcher` on-disk cache to avoid downloading Chromium twice
- **`noteey_to_text`** in `src/parse/mod.rs` strips `MM:SS` / `HH:MM:SS` timestamp prefixes from noteey-style transcripts, drops marker-only lines like `[Music]` and `(Applause)`, replicates the 50 MiB safety cap from `srt_to_text`, and normalises Unicode to NFC
- **`SubtitleFormat` enum** with `Srt | NoteeyTranscript` variants and `SubtitleInfo::format_hint` field for dispatch. `Srt` is `#[default]` so existing providers and consumers are unaffected
- **`convert_format`** in `src/commands/mod.rs` now accepts a `format_hint` parameter and picks the right parser (`srt_to_text` for SRT, `noteey_to_text` for noteey transcripts). Rejects `--format srt` when the only available source is noteey via `AppError::InvalidUsage` with a clear "use --format txt" message — noteey has no SRT framing so we do not fabricate timestamps
- **`ProviderChoice::ProviderNoteey`** variant in `src/cli.rs` so operators can pin to the noteey-only path with `--provider provider-noteey`. TOML config accepts the same `provider-noteey` value. The `provider-noteey-wiremock` integration test mirrors the existing `provider-headless-wiremock` pattern
- **`src/provider/stealth.rs`** shared anti-fingerprint module (GAP-AUD-2026-041 + GAP-AUD-2026-044). Exposes `pub async fn apply_stealth(page: &Page)` which queues `STEALTH_INIT_JS` via the CDP `Page.addScriptToEvaluateOnNewDocument` call. The init script masks `navigator.webdriver`, pollutes `navigator.plugins` with 3 default-Chromium entries, overrides `navigator.languages`, swaps the `SwiftShader` WebGL vendor for `Intel Inc.`, and installs a minimal `chrome.runtime` mock. 5 inline tests pin the script's content; the test against a live `chromiumoxide::Page` is left to manual verification against `https://fingerprintjs.github.io/fingerprintjs/` (see `stealth.rs` doc comment)

### Fixed
- **GAP-AUD-2026-038** (auditoria e2e de 2026-06-19): `provider_headless.rs` now returns `ProviderUnavailable` (exit 69) when downsub.com is degraded — specifically when the JS reports `no matching button` after 45 polls, when a matched button has no `href`, when the per-language button fetch returns non-200, or when an unknown JS-level error is surfaced. `NoSubtitle` (exit 66) is now reserved for the genuine "button found, status 200, body empty" case where downsub confirmed the video has no subtitles. The chain's `saw_no_subtitle` winner rule (`src/provider/mod.rs:270-273`) now correctly falls through to `provider-noteey` when downsub reports degradation
- **GAP-AUD-2026-039** (auditoria e2e de 2026-06-19): `ProviderOutcome` enum (internal to `ProviderChain`) classifies each provider response as `Subtitle(info, bytes)` or `ChainError { source, error, degraded }`. HTTP 5xx and HTTP 429 are marked `degraded = true` — the chain walks past them without recording `last_err` or marking `saw_no_subtitle`. The `Provider` trait remains unchanged; classification happens inside the chain wrapper around `fetch_subtitle`/`fetch_content`. Effect: `provider_a` and `provider_b` returning transient 5xx/429 no longer poison the chain with `NoSubtitle`, so the fallback to `provider-headless` and `provider-noteey` now triggers correctly from any chain position. 3 new chain tests cover the degraded-skip contract
- **GAP-AUD-2026-040** (preventivo): `noteey_to_text` enforces the same 50 MiB cap as `srt_to_text` to prevent OOM on long-form videos
- **GAP-AUD-2026-041** (auditoria e2e de 2026-06-19): `provider_noteey.rs::drive_page` now calls `stealth::apply_stealth(&page)` immediately after `browser.new_page("about:blank")` and before `page.goto(...)`. The CDP `Page.addScriptToEvaluateOnNewDocument` patches mask the headless-Chromium signals that Cloudflare Windsor.io (`r.wdfl.co/rw.js`) fingerprints in the first document load. Without this patch, noteey.com assigns a high risk score and blocks session creation in datacenter IPs
- **GAP-AUD-2026-044** (auditoria e2e de 2026-06-19, PRIORITY): `src/provider/stealth.rs` is the new shared anti-fingerprint module. `apply_stealth(page)` is invoked from BOTH `provider_headless::drive_page` (downsub.com) and `provider_noteey::drive_page` (noteey.com) BEFORE the first `page.goto`. The init script patches 5 fingerprint signals: `navigator.webdriver`, `navigator.plugins`, `navigator.languages`, `WebGLRenderingContext.prototype.getParameter` (masks `SwiftShader` vendor string), and `window.chrome.runtime`. Companion Chromium flag `--disable-blink-features=AutomationControlled` is now in both providers' `BrowserConfig`. The `HEADLESS_NAV_TIMEOUT = 60s` constant moved from each provider into `stealth.rs` to eliminate silent drift between the two. This is the unified root cause for the Cloudflare fingerprinting issue observed across both headless providers — closing this gap unlocks the downsub→noteey fallback in datacenter environments

### Fixed (v0.3.2)
- **GAP-AUD-2026-045**: terminal CDP errors in `provider_noteey.rs` submit/extract now return `ProviderUnavailable` instead of `Internal`, enabling automatic chain fallback when the CDP target closes during Cloudflare challenges
- **GAP-AUD-2026-046**: `noteey_extract_diagnostic` now dumps first 500 chars of page body via `tracing::warn!` when polling exhausts without finding transcripts, enabling operators to distinguish captcha interstitial, empty page, and partial render without re-executing manually
- **GAP-AUD-2026-054**: chain no longer masks `BrowserNotFound` behind `NoSubtitle(NotPublished)`. The `remember_failure` helper now protects `BrowserNotFound` and `CaptchaChallenge` signals so operators receive exit 69 with an actionable "chromium/chrome not found" message instead of exit 66
- **GAP-AUD-2026-055**: `prepare_user_data_dir()` in `stealth.rs` sweeps stale `Singleton{Lock,Cookie,Socket}` files before `Browser::launch`, auto-curing the abort caused by orphan locks from prior crashes. Browser profile anchored to `~/.cache/youtube-legend-cli/chrome-profile/` instead of global `/tmp/chromiumoxide-runner/`

### Changed (noteey surface, historical — before provider removal)
- `provider_headless::ensure_chrome` is now `pub` so `provider-noteey` can share the same Chromium pin and on-disk `BrowserFetcher` cache directory

## [0.3.1] - 2026-06-19

### Added
- CLI flag `--headless` to force the headless browser fallback in `auto` mode
- `AppError::BrowserNotFound(String)` variant with exit code 69 (EX_UNAVAILABLE) and a human-readable installation hint
- `BrowserFetcher` auto-download fallback in `provider_headless.rs` when no local Chrome/Chromium is found (downloads to `$XDG_CACHE_HOME/youtube-legend-cli/browser/`)
- 60s `tokio::time::timeout` wrapping `Browser::new_page` and `page.wait_for_navigation` to surface Cloudflare challenge latency as `AppError::Timeout`
- `YT_LEGEND_NO_NETWORK` honored by `ProviderHeadless::fetch_subtitle` (returns `ProviderUnavailable` without spawning the browser)
- `chromiumoxide` features `["fetcher", "rustls", "zip8"]` enabled in `Cargo.toml` so the `BrowserFetcher` API is reachable
- Integration test `tests/integration/provider_headless_wiremock.rs` covering the env-var short-circuit and builder contract

### Fixed
- `provider_headless.rs` no longer reports a generic `ProviderUnavailable` when Chrome is missing — surfaces a structured `BrowserNotFound` message with install instructions
- `Cli::validate()` now rejects `--headless` at parse time when the binary was not built with `--features headless`
- **GAP-AUD-001**: Config file parse and IO errors now return exit code 78 (`EX_CONFIG`) instead of 64 (`EX_USAGE`), aligning with `rules-rust-cli-stdin-stdout-config-observabilidade`. The new `AppError::Config(String)` variant carries the file path in its `Display` message
- **GAP-AUD-002**: Documented exit code 2 from `clap::Error::exit()` for invalid argument parsing (e.g. `--lang xx`). Code 2 is the canonical clap behavior per `rules-rust-cli-stdin-stdout-clap-exitcodes-erros` and is intentionally distinct from 64 (`AppError::InvalidUsage` for post-parse validation failures)
- **GAP-AUD-003**: `ProviderHeadless` now pins `BrowserFetcher` to Chromium revision 1378488 (compatible with `chromiumoxide` 0.9.1's `Message` enum) via `with_version(BrowserVersion::Revision(Revision::new(1378488)))`, eliminating the `WS Invalid message: data did not match any variant of untagged enum Message` warning loop during navigation

## [0.3.0] - 2026-06-15

### Added
- ProviderYouTubeDirect (`src/provider/provider_youtube_direct.rs`) — GAP-001 M4 — native YouTube provider that queries the public `ytInitialPlayerResponse` and `captionTracks[].baseUrl` endpoint without relying on third-party services
- Module `src/provider/youtube/` with:
  - `player_response.rs` (M1): parser of `ytInitialPlayerResponse` extracted from the watch page via regex
  - `player_js.rs` and `decipher.rs` (M3): signature decipher ported from `base.js` with XDG cache
  - `ncode.rs` (M3.5): n-parameter permutation for protected videos
  - `caption_track.rs`: domain type for caption tracks
- Srv3/Json3 parser in `src/parse/srv3.rs` (M2): converts native YouTube formats to SRT
- Binary `youtube-direct-probe` in `src/bin/` for diagnostics
- Test fixtures: `tests/fixtures/player/base_v123.js`, `tests/fixtures/player/ncode_v456.js`, `tests/fixtures/timedtext/sample_{en.srv3,multiline.srv3,pt.json3}`
- New errors in `src/error.rs`: `SignatureDecipherFailed`, `PlayerResponseMissing`, `CaptionTrackNotFound`, `TimedtextUpstreamError`
- 196+ green tests (increment of ~30 tests from GAP-001)

### Changed
- Refactor: `src/cache.rs` became `src/cache/` (mod operations_cache, player_js_cache) — M3 of GAP-001 introduced XDG cache for player `base.js`

### Fixed
- META-GAP-B: DoS protection in `player_response.rs` — added `MAX_JSON_DEPTH: usize = 64` guard via byte-level scanner before `serde_json::from_str` (see `gaps.md` META-GAP-B)

### Documentation
- Skill manifest reescrito: `skill/youtube-legend-cli-en/SKILL.md` (369 linhas) e `skill/youtube-legend-cli-pt/SKILL.md` (366 linhas). 16 lacunas cobertas: provider chain com `provider-headless` 4o opcional, 17 flags adicionadas/refinadas (`--asr`, `--no-fallback`, `--dry-run`, `--batch`, `--format`, `--cache-ttl`, `--no-cache`, `--config`, `--no-progress`, `--yes`, `--user-agent`, `--timeout`, `--verbose`, `--quiet`, `--log-level`, `--log-format`, `--color`), JSON envelope com `meta` block (`provider`, `captions_url`, `deciphered_signature`), binary `youtube-direct-probe` com exemplo NDJSON, binary `snapshot`, 4 novas error variants (`SignatureDecipherFailed`, `PlayerResponseMissing`, `CaptionTrackNotFound`, `TimedtextUpstreamError`) todas exit 70, M1-M5 marcos do GAP-001, modulo `provider::youtube`, `srv3` parser, `player_js_cache` XDG 7-day TTL, 6 cross-compile targets, env vars `YT_LEGEND_CACHE_DIR` e `YT_LEGEND_NO_NETWORK`, feature `headless`, `provider_youtube_direct` struct path, test fixtures, See Also para `CHANGELOG.md` e `docs/MIGRATION.md`. 9/9 auditorias de conformidade PASS.

## [0.2.8] - 2026-06-14

### Fixed
- `rust-version` in `Cargo.toml` over-declared `1.96.0` while the codebase compiles and tests cleanly on `1.88.0`. Lowered to `1.88.0` so users on stable Fedora (rustc 1.94.1) and the wider 1.88+ ecosystem can `cargo install youtube-legend-cli` without the cargo client refusing the manifest. The local `rust-toolchain.toml` remains on the latest stable (1.96.0) for reproducible development; the contract with end users lives in `Cargo.toml` `rust-version`.

## [0.2.7] - 2026-06-14

### Fixed
- crates.io metadata: `web-programming::scraping` category slug was deprecated; reduced to `["command-line-utilities"]` only.

## [0.2.6] - 2026-06-14

### Added
- Seven new global CLI flags: `--config <PATH>`, `--log-level <LEVEL>`, `--log-format <FORMAT>`, `--color <WHEN>`, `--no-progress`, `--dry-run`, `--yes` (resolves CLI ergonomic gaps for scripting, daemonised use, and log ingestion). See `src/cli.rs` and `src/logging.rs`.
- `mimalloc` as the global allocator in `src/main.rs` to reduce allocation overhead in the subtitle-fetching hot path (HTTP body buffers, URL parsing).
- Criterion-based benchmark target via `cargo bench --bench cache_bench`. Three micro-benchmarks cover the cache key composer, URL length check, and BCP 47 locale parser. Run with `cargo bench --bench cache_bench`; CI verifies the target compiles via `cargo bench --no-run`.
- `headless` Cargo feature gating an optional headless-browser fallback provider (`src/provider/provider_headless.rs`). Drives a local Chromium/Chrome instance via `chromiumoxide` to run the upstream site's own JavaScript and download the subtitle through the page's same-origin session, recovering downloads when the plain HTTP providers are blocked by Cloudflare or browser-gated endpoints. Disabled by default; enable with `cargo build --features headless`. Resolves the browser via `$CHROME` or well-known install paths.
- `robots.txt` compliance for both providers (NFR-007, GAP-010). Provider fetchers consult `src/provider/robots.rs` before issuing any request to the upstream host; the `Disallow` path is treated as `EX_UNAVAILABLE` so downstream tooling can branch on the same exit code as a network failure.
- Offline-cache integration test `tests/integration/offline_cache.rs` (NFR-005, GAP-008) exercising the `(read cache, no network, plain text output)` round-trip with a fixture URL.
- `tests/integration/rss.rs` enforcing the NFR-002 RSS budget of 100 MiB during integration runs.
- `tests/integration/provider_a_wiremock.rs` and `tests/integration/provider_b_wiremock.rs` exercising both providers against `wiremock` mocks so the binary can be tested without ever touching the live upstream (GAP-012).
- `tests/integration/signal_handler_stress.rs` (gated `#[ignore]`) for `SIGINT` / `SIGTERM` behaviour under stress. Local-run only.
- `AppError::RateLimited` for upstream HTTP 429 responses, carrying the parsed `Retry-After` delta-seconds; the retry layer honours it with a 60 s fallback capped at 300 s (EC-021). A rate-limit error from one provider is preserved across the chain even when a later provider fails generically.
- Every retry attempt is logged as a structured `event = "retry"` tracing event with the attempt number and next delay (FR-013).
- `Retry-After` is now also honoured in RFC 2822 HTTP-date form, converted to delta-seconds against the current clock and clamped to zero when the date is in the past (EC-021, clock-skew safe).
- `#[tracing::instrument]` on 14 internal public-API entry points (GAP-011) — `commands::run`, `extract::run`, `batch::run`, `ProviderA::fetch_subtitle`, `ProviderB::fetch_subtitle`, `ProviderChain::fetch_subtitle`, `cache::{read,write,path}`, `retry::retry_with_backoff`, `parse::{extract_video_id,srt_to_text}`, `io::read_url_from_stdin`, `commands::batch::dedup_set` and the new `provider::robots::check`.
- Heuristic-fallback extractor for provider-A HTML drift (EC-024, GAP-023): when the primary `scraper::Html` selector fails, the fetcher now also walks the `JSON-LD VideoObject` block on the page for caption URLs.
- Cross-compile target matrix now covers six targets via `ci.yml` `cross-compile` job (GAP-022, GAP-024): `x86_64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `x86_64-pc-windows-msvc`, `x86_64-apple-darwin`, `aarch64-apple-darwin` (last two with `continue-on-error: true` because they require `osxcross`).
- `tests/integration/io.rs` covers `read_url_from_stdin` and `read_urls_from_stdin` (GAP-026) for the three input shapes (single URL, batch via `--batch`, one-URL-per-line).
- `src/bin/snapshot.rs` companion binary (`cargo run --bin snapshot`) probes both providers and writes redacted HTML snapshots under `tests/fixtures/snapshots/<date>/` for drift detection. The `secret_endpoints` module is consumed via `#[path = "../secret_endpoints.rs"] mod secret_endpoints;` so the public crate API never re-exports the host constants.
- `docs/agent-teams-workflow.md` — playbook for the Agent Teams workflow (plan / spawn / validate / cleanup), with the four known break modes recorded as the workflow matures.
- `docs/decisions/0009-cargo-toml-ownership-in-parallel-tasks.md` — ADR-0009 records the rule that `Cargo.toml` is owned by exactly one task per session under the Agent Teams workflow, after the 2026-06-13 mimalloc/Cargo.toml drift incident.
- `--lang` now accepts BCP 47 language tags (`pt-BR`, `pt_BR.UTF-8`, `EN-us`); malformed or unsupported locales are rejected with exit code `2` (FR-009).
- `[package.metadata.docs.rs]` `targets` now includes both `x86_64-apple-darwin` and `aarch64-apple-darwin` (was 4 targets, now 6).
- Crate-level rustdoc, `//!` module docs, and `///` item docs throughout the public API.
- `cargo doc --no-deps --all-features` and `cargo test --doc` to CI.
- `cargo-deny`, `cargo-audit`, `cargo-public-api` (with sigilo gate), `cargo-semver-checks`, `lychee`, and the Agent-Teams matrix-os and nightly jobs in CI.
- `LICENSE` (dual MIT / Apache-2.0), `CONTRIBUTING.md`, `SECURITY.md`, and `CODE_OF_CONDUCT.md`.
- `clippy.toml` with 3 disallowed methods, `cognitive-complexity-threshold = 30`, `too-many-arguments-threshold = 8` (GAP-019).
- `rustfmt.toml`, `rust-toolchain.toml`.
- `[badges.maintenance]` in `Cargo.toml`.
- Three runnable examples in `examples/`: `single_url`, `batch`, and `json_output`.
- `#[non_exhaustive]` on every public enum.

### Fixed
- Provider-B API request was built from a scheme-less host constant, so `reqwest` rejected it at send time with an opaque builder error and the POST never left the process; the URL is now constructed with an explicit `https://` scheme. A regression test asserts the API URL parses as an absolute HTTPS URL.
- `default-run` is now set so `cargo run` and the integration corpus harness resolve the CLI binary unambiguously.
- Provider-B AJAX endpoint path corrected after the provider renamed it; the previous path returned HTTP 404.
- Provider-B now discovers its AJAX endpoint path from the page's inline JavaScript at runtime, with the compiled-in path as a fallback, so the client adapts automatically when the provider renames the endpoint.
- **BREAKING**: exit codes migrated from the legacy 2-7 scheme to BSD `sysexits.h` (64-78). Mapping: invalid usage/input `2` → `64` (`EX_USAGE`); invalid URL `3` → `65` (`EX_DATAERR`); no subtitle `4` → `66` (`EX_NOINPUT`); all providers unavailable or rate limited `5` → `69` (`EX_UNAVAILABLE`); HTTP timeout `6` → `70` (`EX_SOFTWARE`); I/O, HTTP, serde, crypto, subtitle-too-large, and internal errors `7` → `70` (`EX_SOFTWARE`). Pipelines that switched on the exact legacy code must update their branches. See `src/error.rs::sysexits` and the README exit-code table for the canonical reference.
- `secret_endpoints` was exposed as `pub mod` on the public crate root, leaking the upstream hostnames via rustdoc; the module is now `pub(crate) mod secret_endpoints;` and the `snapshot` binary consumes it via `#[path = "../secret_endpoints.rs"]` (GAP-007). A CI gate (`ci.yml` `public-api` job) fails the build if `pub mod secret_endpoints` ever regresses.
- `pub use` surface on the public crate root reduced from 14+ symbols down to 2 justified re-exports (`Cli`, `FormatArg`, `LanguageArg`, `AppError`, `AppResult`, `NoSubtitleReason`) (GAP-017).
- `text` module is now `pub(crate) mod text` instead of `pub mod text` since Unicode NFC normalisation is an internal helper (GAP-018, GAP-027).
- `cargo.toml` exclude list extended to cover `docs_prd/**`, `docs_rules/**`, `.github/**`, `tests/fixtures/snapshots/**`, `*.bak.*`, `*.tar.gz`. Prevents build artefacts and personal working files from being packaged into the published crate.
- `// SAFETY:` comments added to all `unsafe` blocks in `cache.rs`.
- `pub use` on `cli` and `error` modules re-asserted; removed the dangling `text` re-export.
- Stale `src/*.bak.*` files removed from the working tree; only `tests/fixtures/snapshots/` and `Cargo.toml.bak.*` (if any are re-introduced by tools) are now tolerated.

### Changed
- Dependency majors: `thiserror` 1.0 → 2.0, `scraper` 0.20 → 0.27, `rand` 0.8 → 0.10 (`OsRng`/`RngCore` migrated to `SysRng`/`TryRng` with explicit error propagation), `reqwest` 0.12 → 0.13 (`rustls-tls` feature renamed to `rustls`; `form` is now an explicit opt-in feature).
- All `#[error("...")]` messages in `error.rs` translated to English.
- `io.rs` and `parse/video_id.rs` error and info messages translated to English.
- CLI `help` and `about` strings translated to English; test assertions updated to match.
- `snapshot` binary `about` and redaction-notice messages translated to English.
- `Cargo.toml` `description` translated to English; `rust-version` pinned to `1.96.0`; dead `anyhow` dependency removed.
- `ci.yml` extended with `deny`, `audit`, `public-api` (with sigilo gate), `semver-checks`, `cargo-install`, `matrix-os`, `nightly`, `docs-link-check`, doctest, and doc-build jobs.
- README rewritten in English with badges, flags table, exit-code table, install instructions, performance baseline, and a Documentation section.
- llms.txt and llms-full.txt brought into line with the BSD exit codes, the 17 wired flags, the 6 cross-compile targets, and the current public API surface (modules, `pub use` set).
- `CONTRIBUTING.md` rewritten with a clean bash code block, the `atomwrite` and `agent-teams-workflow` references, and the 8 quality gates.

### Removed
- 26 stale `.bak` backup files from `src/`.
- Orphaned `BSD-2-Clause` allowance from `deny.toml` (no dependency uses it after the upgrades below).

## [0.1.0] - 2026-06-01

### Added
- Initial public release.
- Single-binary Rust CLI for downloading YouTube subtitles.
- Support for four YouTube URL forms: `watch`, `shorts`, `embed`, `youtu.be`.
- Two-provider extraction pipeline with automatic fallback.
- Structured JSON output via `--json`.
- Batch mode reading URLs from stdin via `--batch`.
- Verbose mode emitting `tracing` events to stderr.
- Local file cache with 24-hour TTL under `~/.cache/youtube-legend-cli/`.
- Exponential-backoff retry (1 s, 2 s, 4 s) for transient failures.
- In-memory circuit breaker per provider.
- AES-256-CBC + PBKDF2-HMAC-SHA1 (100 iterations) token encryption.
- 50 MiB in-memory safety cap on decoded subtitle size.
- GitHub Actions CI: format check, clippy, build, test, cross-compile, publish dry-run, MSRV.
- Bilingual `pt-BR` / `en-US` user-facing README.

## [0.0.1] - 2026-05-15

### Added
- Initial PRD with 13 mandatory sections.
- Reverse-engineering notes from live traffic.
- 22 `rules_rust` documents under `docs_rules/`.
- Constitution with 15 principles `PRINC-001` through `PRINC-015`.
- Technical spec covering 14 modules.
- Implementation plan in 8 phases.
- 4 corpus URLs in `tests/fixtures/corpus.txt`.

[0.3.3]: https://github.com/daniloaguiarbr/youtube-legend-cli/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/daniloaguiarbr/youtube-legend-cli/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/daniloaguiarbr/youtube-legend-cli/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/daniloaguiarbr/youtube-legend-cli/compare/v0.2.9...v0.3.0
[0.2.8]: https://github.com/daniloaguiarbr/youtube-legend-cli/compare/v0.2.7...v0.2.8
[0.2.7]: https://github.com/daniloaguiarbr/youtube-legend-cli/compare/v0.2.6...v0.2.7
[0.2.6]: https://github.com/daniloaguiarbr/youtube-legend-cli/compare/v0.1.0...v0.2.6
[0.1.0]: https://github.com/daniloaguiarbr/youtube-legend-cli/releases/tag/v0.1.0
[0.0.1]: https://github.com/daniloaguiarbr/youtube-legend-cli/releases/tag/v0.0.1

## [0.2.9] - 2026-06-14

### Added
- `docs/ARCHITECTURE.md` (mermaid pipeline diagram, module map, stream
  contract, provider pipeline, cancellation, MSRV section) and
  `docs/decisions/0010-deferred-doc-cfg-migration.md` (MADR-format
  ADR explaining why the `doc_auto_cfg → doc_cfg` migration is
  deferred to v0.3.0).
- Centralised `[lints.clippy]`, `[lints.rust]`, and `[lints.rustdoc]`
  tables in `Cargo.toml` covering 12 official rustdoc lints plus
  `clippy::doc_markdown`, `clippy::missing_errors_doc`,
  `clippy::missing_panics_doc`, and `clippy::missing_safety_doc`.
  The duplicated `#![warn/deny(...)]` block in `src/lib.rs` was
  removed in favour of the single source of truth.
- Expanded `#[doc(alias = "...")]` surface on `Cli`, `AppError`, and
  the `Provider` trait to cover the SEO queries that would have been
  served by the still-unstable `#[doc(keyword = "...")]` attribute.

### Fixed
- 18 clippy errors caught by the new centralised lints:
  `clippy::doc_markdown` (15 missing-backticks in module- and
  struct-level doc comments) and `clippy::missing_errors_doc` (3
  `Result`-returning functions without a `# Errors` section) were
  fixed across `src/lib.rs`, `src/cli.rs`, `src/commands/mod.rs`,
  `src/error.rs`, `src/parse/video_id.rs`, `src/provider/mod.rs`,
  `src/provider/provider_a.rs`, `src/provider/provider_b.rs`,
  `src/retry.rs`, `src/bin/snapshot.rs`, and
  `src/secret_endpoints.rs`.
- `llms.txt` and `llms-full.txt` now point at
  `github.com/daniloaguiarbr/youtube-legend-cli` (the user-facing
  GitHub handle) instead of the stale `github.com/danilo/`. The
  `## Docs` section in `llms.txt` is renamed to `## Documentation`
  to match the llmstxt.org spec, and a new `## Architecture`
  section summarises the pipeline for LLM consumers. The
  `web-programming::scraping` category slug (deprecated by
  crates.io since v0.2.7) is dropped from the `llms-full.txt`
  `Categories` line.
- Three stale `ci.yml.bak.*` snapshots in `.github/workflows/`
  were removed (they were already covered by the `*.bak.*`
  patterns in `.gitignore`).

### Changed
- `cargo clippy --all-features -- -D warnings`,
  `cargo doc --no-deps --all-features`, and
  `RUSTDOCFLAGS="-D warnings" cargo doc --all-features` now all
  exit clean. This is the new quality bar enforced by CI.

### Fixed
- **GAP-AUD-2026-049** (auditoria e2e de 2026-06-19, CRÍTICO): `ProviderChain::fetch_subtitle` em `src/provider/mod.rs:412-434` usava `provider.fetch_subtitle(...).err()` dentro do match arm para recuperar a variante do erro. Para `provider-headless` e `provider-noteey`, isso re-invocava o provider inteiro, fazendo `Browser::launch` rodar DUAS vezes por chamada (segundo spawn do chromiumoxide). Resultado: latência dobrou de ~30s para ~60s em chamadas degradadas. Corrigido via pattern guard `Err(e @ (AppError::ProviderUnavailable | RateLimited | CaptchaChallenge))` que captura o erro já disponível sem re-invocar o provider. O erro é boundado a `e` no match guard e reutilizado direto no `ProviderOutcome::ChainError`
- **GAP-AUD-2026-050** (auditoria e2e de 2026-06-19): envelope JSON documentado em `docs/AGENTS.pt-BR.md` divergia do contrato emitido. Faltava campo `provider` (estável, casando com `Provider::name()`) e os campos `bytes` e `source` tinham nomes divergentes da documentação (`byte_size` e `source_url`). `SubtitleInfo` ganhou campo `pub provider: &'static str` em `src/provider/mod.rs:218` populado por todos os 5 providers (`youtube-direct`, `provider-a`, `provider-b`, `provider-headless`, `provider-noteey`). `JsonSuccess` struct em `src/commands/mod.rs:14-32` agora inclui `provider`, `byte_size`, `source_url`. `#[non_exhaustive]` na `SubtitleInfo` garante retrocompatibilidade
- **GAP-AUD-2026-051** (auditoria e2e de 2026-06-19, CRÍTICO): cache hit em `src/commands/extract.rs:54-58` (pré-fix) hardcodava `SubtitleFormat::Srt` como `format_hint` ao chamar `convert_format`. Quando o cache continha bytes do `provider-noteey` (linhas com prefixo `MM:SS`), o parser `srt_to_text` não removia os timestamps — saída era `00:01 [música] Opa, hoje é o seu dia de sorte. 00:10 sabe como é que...` em vez de texto limpo. Corrigido via sidecar `*.hint` em `src/cache/mod.rs:151-179, 199-226`. Funções novas `read_cache_with_hint` e `write_cache_with_hint` persistem o discriminator (`srt` ou `noteey-transcript`) em UTF-8 no sidecar. Retrocompatível: cache v0.3.0 sem sidecar retorna `SubtitleFormat::Srt` conservadoramente. `commands::extract` e `commands::batch` ambos atualizados
- **GAP-AUD-2026-045** (auditoria e2e de 2026-06-19): terminal CDP error (`-32000: target navigated or closed`) em `submit_evaluate` ou `extract_evaluate` agora retorna `AppError::ProviderUnavailable` (degraded) em vez de `AppError::Internal`. Implementado em `provider_noteey.rs:321-334, 371-380` e `provider_headless.rs:588-602` via `is_terminal_cdp_error` classification. O chain agora faz fallback automático para o próximo provider quando o target CDP morre durante a sessão, em vez de tentar retry contra o mesmo target morto
- **GAP-AUD-2026-046** (auditoria e2e de 2026-06-19): `noteey_extract_diagnostic` em `provider_noteey.rs:394-410` emite primeiros 500 chars do body quando o polling esgota sem encontrar transcripts renderizados. Permite distinguir captcha interstitial (CF Turnstile), página vazia (degradação upstream) e render parcial (DOM lento) via tracing estruturado. Operador pode grep `noteey_extract_diagnostic` para diagnóstico de falhas
- **GAP-AUD-2026-047** (auditoria e2e de 2026-06-19, regressão): `noteey_to_text` em `src/parse/mod.rs:245-339` ganhou 11 testes de regressão cobrindo MM:SS, HH:MM:SS, Millis, UTF-8 NFC, marcadores `[...]`, cap 50 MiB, linhas vazias duplicadas. Previne reintrodução de bugs em mudanças futuras. O bug original era `noteey_to_text` retornando timestamps em vez de texto limpo quando o body tinha formato misto
- **GAP-AUD-2026-048** (auditoria e2e de 2026-06-19): `findTranscriptRegion` em `provider_noteey.rs:78-93` isola o transcript pane via seletor estável (`[data-transcript]`, `[class*="transcript" i]`, `[id*="transcript" i]`) com fallback heurístico para o menor container com ≥3 timestamps. Sem esta isolation, o body enviado ao parser Rust incluía o header da página (nav, hero, login button), vazando ruído no output
- **GAP-AUD-2026-054** (auditoria e2e de 2026-06-19, CRÍTICO): `ProviderChain::fetch_subtitle` em `src/provider/mod.rs:412-522` colapsava para `NoSubtitle(NotPublished)` quando providers estáticos confirmavam ausência de legendas E providers headless/noteey falhavam por falta de chromium no ambiente. O operador recebia exit 66 sem indicação de que precisava instalar chrome. Bug crítico descoberto via compilação local + e2e fresh contra `https://youtu.be/TTEFo3XQYls`. Três correções aplicadas: (1) `AppError::BrowserNotFound(_)` adicionado ao match arm degraded para que chrome faltando não encerre o chain; (2) `remember_failure` agora protege `BrowserNotFound` e `CaptchaChallenge` em adição a `RateLimited`, garantindo que sinais de ambiente sobrevivam em `last_err`; (3) match arm final do chain prefere sinais de ambiente (`BrowserNotFound`/`CaptchaChallenge`/`RateLimited`) sobre `NoSubtitle` consolidado. Resultado e2e: `exit 69 EX_UNAVAILABLE` com mensagem completa `chromium/chrome not found: ... Set $CHROME or install chromium-browser / google-chrome`. Dois testes atualizados em `src/provider/mod.rs:760-781` e `:935-950` para refletir a nova precedência (EC-021: RateLimited vence NoSubtitle consolidado). 250 testes passam, 0 falhando, 1 ignored, clippy zero warnings
- **GAP-AUD-2026-055** (auditoria e2e de 2026-06-19, RESILIÊNCIA): `BrowserConfig::builder()` em `provider_headless.rs:333` e `provider_noteey.rs:205` não setava `user_data_dir`, deixando chromiumoxide 0.9.1 usar o default global `/tmp/chromiumoxide-runner/`. Após crash anterior (SIGKILL/OOM), symlinks `Singleton{Lock,Cookie,Socket}` órfãos faziam o chrome abortar com `Failed to create /tmp/chromiumoxide-runner/SingletonLock: Arquivo existe (17)` e exit 5376. Sintoma: mensagem genérica `Set $CHROME or install chromium-browser` induzia operador a procurar um browser já instalado. Bug descoberto via compilação local + e2e fresh contra `https://youtu.be/wnZGZG1dRtI`. Três correções aplicadas: (1) novo helper `prepare_user_data_dir()` em `src/provider/stealth.rs:128` ancora o profile em `~/.cache/youtube-legend-cli/chrome-profile/` (XDG cache do projeto, isolado de outros usuários do chromiumoxide); (2) sweep de singleton locks órfãos via `kill -0 <pid>` antes do `Browser::launch` (sem dependência nova); (3) mensagem de erro em `Browser::launch.map_err` distingue `Timeout while resolving websocket`/`Connection error` (CDP mismatch entre chromiumoxide 0.9.1 e Chromium 149 do Fedora 44+) da mensagem genérica. Resultado e2e: exit 69 com mensagem acionável `chromiumoxide 0.9.1 cannot speak CDP with this Chromium build — check $CHROME points at the BrowserFetcher-pinned revision, or use a static HTTP provider via --provider provider-a / provider-b`. 250 testes passam, 0 falhando, 1 ignored, clippy zero warnings
