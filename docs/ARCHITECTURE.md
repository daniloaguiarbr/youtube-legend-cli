# Architecture — youtube-legend-cli

Last reviewed: 2026-06-15 (audit pre-v0.3.0)
Scope: high-level view of the crate, intended for newcomers and
LLM-assisted contributors. The full rustdoc on docs.rs is the
authoritative reference; this file is the map.

## Bird's-eye view

`youtube-legend-cli` is a single-binary CLI that turns a YouTube URL
into a clean subtitle file. It speaks a native Unix `stdin`/`stdout`
contract, exposes a JSON envelope via `--json`, and never blocks on a
TUI or a prompt.

```mermaid
flowchart LR
  A[CLI args / stdin] --> B[Cli::parse]
  B --> C[commands::run]
  C --> D[ProviderChain]
  D -->|M4| Y[ProviderYouTubeDirect]
  D -->|A| E[ProviderA]
  D -->|B| F[ProviderB]
  D -->|headless feature| G[ProviderHeadless]
  E --> H[retry::retry_with_backoff]
  F --> H
  G --> H
  H --> I[cache::read / write]
  H --> J[SubtitleInfo + body]
  J --> K[stdout or --json]
```

## Module map

| Module | Role | Re-exported at crate root |
|---|---|---|
| `cli` | clap-derived argument parser, `Cli` struct, 20 flags (17 from v0.2.x plus `--provider`, `--asr`, `--no-fallback` from v0.3.0) | `Cli`, `FormatArg`, `LanguageArg` |
| `commands` | top-level dispatch (`run`, `extract::run`, `batch::run`) | `run` |
| `provider` | `Provider` trait, `ProviderA`, `ProviderB`, `ProviderChain`, `provider::robots`, optional `ProviderHeadless` (feature = `headless`), and `provider_youtube_direct` plus the `provider::youtube` submodule (M1–M5 + M3.5 of GAP-001) | `Provider` only (concrete providers via `provider::*`) |
| `provider::youtube` | `player_response` (M1 parser of `ytInitialPlayerResponse`), `player_js` and `decipher` (M3 signature decipher with XDG cache), `ncode` (M3.5 n-parameter permutation), `caption_track` (domain type) | via `provider::youtube::*` |
| `parse` | `extract_video_id`, `srt_to_text`, and `srv3` (M2: Srv3/Json3 to SRT) | via `parse::*` |
| `cache` | TTL-keyed local file cache at `~/.cache/youtube-legend-cli/` plus player.js XDG cache (M3); reorganized from flat `src/cache.rs` to `src/cache/` (operations_cache, player_js_cache) | via `cache::*` |
| `retry` | `retry_with_backoff`, `CircuitBreaker` | via `retry::*` |
| `io` | stdin/stdout/TTY helpers | via `io::*` |
| `error` | `AppError`, `AppResult`, `NoSubtitleReason` | `AppError`, `AppResult`, `NoSubtitleReason` |
| `logging` | `init_tracing` (EnvFilter precedence) | via `logging::*` |
| `crypto` | AES-256-CBC + PBKDF2 for provider-B signing | via `crypto::*` |
| `text` | Unicode NFC normalisation | `pub(crate)` only |
| `secret_endpoints` | upstream hostnames and tokens | `pub(crate)` only (consumed by `src/bin/snapshot.rs` via `#[path = "..."]`) |
| `bin::youtube-direct-probe` | diagnostic companion that exercises `ProviderYouTubeDirect` against a single URL for live debugging of the M1–M5 path | binary, not re-exported |

## Stream contract

- `stdout` is reserved exclusively for the subtitle body (or the
  `--json` envelope).
- `stderr` is reserved exclusively for logs, progress, and human
  error messages.
- `stdin` accepts a single URL, a batch of one URL per line, or
  `--batch` flag input.

## Provider pipeline

1. `provider::robots::check` consults the upstream `robots.txt` and
   short-circuits with `EX_UNAVAILABLE` on `Disallow`.
2. `ProviderChain` walks `ProviderYouTubeDirect` first (M4: direct
   YouTube watch page and `captionTracks[].baseUrl`), then `ProviderA`,
   then `ProviderB`, and finally `ProviderHeadless` if the `headless`
   feature is enabled. The chain is throttled to one request per second.
3. `retry::retry_with_backoff` wraps each call with three attempts
   at 1 s, 2 s, 4 s. The `Retry-After` header is honoured in both
   delta-seconds and RFC 2822 date form, with a 60 s fallback capped
   at 300 s. A `429` response from any provider raises
   `AppError::RateLimited`.
4. A successful `fetch_subtitle` returns a `SubtitleInfo` and a
   body. The body is read back via `fetch_content` and written to
   the user (plain text or SRT) or wrapped in a JSON envelope.

## Cancellation

`SIGINT` and `SIGTERM` are wired through
`tokio_util::CancellationToken` in `main.rs`. In-flight requests are
allowed to complete; the process exits with code 130. The async API
exposed by this crate is cancellation-safe at every public await
point.

## MSRV

`1.88.0` — declared in `Cargo.toml` `rust-version` field. The local
toolchain pinned via `rust-toolchain.toml` may be newer; the MSRV in
`Cargo.toml` is the contract with users.

## DoS Protection in the YouTube Direct Provider

The M1 `player_response` parser uses `serde_json` with an explicit
`arbitrary_limit` cap. Without this, `ytInitialPlayerResponse` can be
arbitrarily large in a hostile response, which lets a single
`GET /watch?v=<id>` exhaust process memory before any retry or
circuit-breaker logic fires. The cap is part of the META-GAP-B fix
tracked in `gaps.md` and applies to all v0.3.0 migrations of the
direct-provider path.

## See Also

- [README](../README.md) — user-facing entry point
- [CHANGELOG](../CHANGELOG.md) — release history
- [llms.txt](../llms.txt) and [llms-full.txt](../llms-full.txt) —
  LLM-friendly excerpts
- [docs/decisions/](decisions/) — ADRs in MADR format
- [docs/agent-teams-workflow.md](agent-teams-workflow.md) — playbook
  used to deliver v0.2.6
- [docs/COOKBOOK.md](COOKBOOK.md) — recipes for scripting the CLI
- [docs/HOW_TO_USE.md](HOW_TO_USE.md) — day-one operator guide
- [docs/TESTING.md](TESTING.md) — test architecture and gates
- [docs/MIGRATION.md](MIGRATION.md) — upgrade notes per release
