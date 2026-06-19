# Changelog
[English](CHANGELOG.md) | [Português Brasileiro](CHANGELOG.pt-BR.md)

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

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
