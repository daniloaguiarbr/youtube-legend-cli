---
name: youtube-legend-cli
description: Triggers when the user asks to download YouTube subtitles, captions, transcripts, SRT, VTT, or any caption file from a YouTube URL. Also triggers on mentions of yt-dlp replacement, daniloaguiarbr, youtube-legend-cli, downsub alternative, save subs, batch subtitle download from a list of YouTube URLs, or the `--provider` selector. Use to invoke the youtube-legend-cli Rust CLI for non-interactive, scriptable, JSON-enveloped subtitle retrieval.
---

## Installation and First Run

### REQUIRED
- Rust toolchain version 1.88 or newer on the host
- Network access to YouTube on TCP/443 for first-run fetches
- Use `cargo install youtube-legend-cli` for any production deployment
- MSRV is 1.88 stable as declared in `Cargo.toml`

### FORBIDDEN
- Do not run `cargo run --release` in a tight loop in production scripts
- Do not build the binary from source in CI when the prebuilt one is available
- Do not use `cargo install --path .` to install a local fork
- Always audit the diff before installing a local fork

### Correct Pattern
```bash
cargo install youtube-legend-cli
youtube-legend-cli "https://youtu.be/NvZ4VZ5hooY" > out.txt
```

## CLI Flags Reference

### REQUIRED
- Pass `--json` for every programmatic consumer
- `--lang` accepts BCP 47 codes (`pt-BR`, `en-US`, `es-ES`)
- `--provider` accepts `auto`, `youtube-direct`, `provider-a`, `provider-b`, or `provider-headless`
- `--asr` forces the auto-generated track, valid only with `youtube-direct`
- `--no-fallback` disables the chain, valid only with `--provider auto`
- `--dry-run` serves from cache and skips the YouTube-direct tier
- `--format` accepts `txt`, `srt`, `vtt`, or `json`
- `--batch` reads URLs from stdin one per line
- `--cache-ttl` accepts positive integer hours to override TTL
- `--no-cache` forces fresh read ignoring the local cache
- `--config <PATH>` loads a TOML configuration file
- `--no-progress` suppresses progress bars on stderr
- `--yes` assumes yes for non-interactive prompts
- `--user-agent` overrides the HTTP User-Agent header
- `--timeout` accepts positive integer seconds for the HTTP bound
- `--verbose` and `--quiet` control log volume on stderr
- `--log-level` accepts `error`, `warn`, `info`, `debug`, `trace`
- `--log-format` accepts `text` or `json`
- `--color` accepts `auto`, `always`, `never`
- Combine `--json` with `--lang` for localized output envelopes

### FORBIDDEN
- Do not hardcode hostnames in scripts
- Do not pass a YouTube URL as positional argument twice
- Do not pass `--no-cache` together with explicit cache invalidation
- Do not combine `--asr` with `provider-a` or `provider-b`
- Do not combine `--no-fallback` with a fixed provider

### Correct Pattern
```bash
youtube-legend-cli --json --lang pt-BR "https://youtu.be/abc" | jaq '.body'
```

## JSON Envelope and Schema

### REQUIRED
- Validate the `error` field on stdout before trusting the body
- Branch on the `code` field matching BSD sysexits
- Read the `retry_after_seconds` field when present
- Pipe stdout through `jaq` or equivalent JSON parser
- The v0.3.0 envelope adds a `meta` block with `provider`, `captions_url`, `deciphered_signature`
- The `deciphered_signature` field is intentionally redacted in the envelope
- Authoritative schema lives at `docs/schemas/caption-track.schema.json`

### FORBIDDEN
- Do not parse stdout line-by-line as raw subtitle text
- Do not skip the envelope check
- Do not assume the body is always a string

### Correct Pattern
```bash
out=$(youtube-legend-cli --json "$url")
echo "$out" | jaq -e '.error == null' >/dev/null || echo "$out" | jaq '.message'
```

## Exit Codes and sysexits.h

### REQUIRED
- `0` for success
- `64` EX_USAGE on invalid arguments
- `65` EX_DATAERR on malformed upstream response
- `66` EX_NOINPUT when the URL has no captions available
- `69` EX_UNAVAILABLE when the provider chain is exhausted
- `70` EX_SOFTWARE on internal failure, including YouTube direct provider errors
- `78` EX_CONFIG on configuration error
- `130` SIGINT on user interrupt

### FORBIDDEN
- Do not rely on exact exit numbers without the category mapping
- Do not mask the exit code with a `|| true` fallback

### Correct Pattern
```bash
youtube-legend-cli "$url" || case $? in
  66) echo "no subs" ;;
  69) echo "upstream down" ;;
  70) echo "internal provider failure" ;;
  *) echo "other failure" ;;
esac
```

## Provider Chain and Selection

### REQUIRED
- Default order: `youtube-direct`, then `provider_a`, `provider_b`, and `provider_headless` when the `headless` feature is on
- The `youtube-direct` provider queries the public YouTube endpoint via `ytInitialPlayerResponse` and `captionTracks[].baseUrl`
- The `Provider` trait is public and instantiated through `provider::ProviderYouTubeDirect` in `src/provider/provider_youtube_direct.rs`
- Pin a specific provider only for deterministic testing
- Document any provider override in the script header

### FORBIDDEN
- Do not pin `provider-a` in CI scripts because it loses the youtube-direct signal
- Do not assume any single provider covers the full catalogue
- Do not confuse the `src/provider/` module with the `src/provider/youtube/` submodule

### Correct Pattern
```bash
# Production lets the chain auto-fallback
youtube-legend-cli --provider auto "https://youtu.be/VIDEO"

# Debug pins a provider and disables fallback
youtube-legend-cli --provider youtube-direct --no-fallback "https://youtu.be/VIDEO"
```

## YouTube Direct Provider (v0.3.0)

### REQUIRED
- The `ProviderYouTubeDirect` provider lives in `src/provider/provider_youtube_direct.rs`
- Helper modules in `src/provider/youtube/` include `player_response.rs`, `player_js.rs`, `decipher.rs`, `ncode.rs`, and `caption_track.rs`
- The player response parser extracts `ytInitialPlayerResponse` from the watch page via regex
- The signature decipher uses the operations table extracted from cached `base.js`
- The n-parameter decipher uses the `ncode` function for protected videos
- The Srv3 and Json3 to SRT conversion happens in `src/parse/srv3.rs`
- The `base.js` cache lives at `~/.cache/youtube-legend-cli/player/<version>.js` with a 7-day TTL
- The `headless` feature remains optional and build-time gated

### FORBIDDEN
- Do not invoke the YouTube direct provider without the `src/provider/youtube/` module
- Do not persist `base.js` outside the XDG cache directory

### Correct Pattern
```bash
youtube-legend-cli --provider youtube-direct --asr --lang pt-BR \
  "https://youtu.be/VIDEO" > subtitle.srt
```

## Cache Behaviour

### REQUIRED
- 24-hour TTL on disk at `~/.cache/youtube-legend-cli/`
- The player JavaScript cache lives at `~/.cache/youtube-legend-cli/player/`
- Use `--no-cache` for fresh fetches in audit pipelines
- Use `--cache-ttl` to override TTL in positive integer hours
- Invalidate a single entry by removing its directory
- The player cache uses single-flight to avoid download storms

### FORBIDDEN
- Do not hardcode `/tmp` paths for cache storage
- Do not delete the entire cache directory in production scripts
- Do not redirect the cache outside XDG

### Correct Pattern
```bash
# Invalidate one entry
rm -rf ~/.cache/youtube-legend-cli/<author>/subtitles/<video>/

# Override TTL for a long-running batch
youtube-legend-cli --cache-ttl 168 "https://youtu.be/VIDEO"
```

## Retry and Rate Limiting

### REQUIRED
- Honour the `Retry-After` header on HTTP 429 responses
- Read the `retry_after_seconds` field from the JSON envelope
- Stop retrying after the envelope-provided delay window
- The internal fallback is 60 seconds with a 300-second cap
- The one-request-per-second throttle is per chain, not per provider

### FORBIDDEN
- Do not run client-side retry loops without backoff
- Do not hammer the same provider after a rate-limit response
- Do not pin `--timeout` below 5 seconds

### Correct Pattern
```bash
# rate-limited errors carry retry_after_seconds in JSON envelope
sleep "$(echo "$out" | jaq '.retry_after_seconds')"
```

## Streaming Contracts

### REQUIRED
- stdout carries subtitle text, SRT, VTT, or JSON envelope only
- stderr carries logs, progress, and diagnostics
- Discard stderr before piping stdout into `jaq`

### FORBIDDEN
- Do not parse logs from stderr as if they were the body
- Do not redirect stderr to a file then re-read it as JSON

### Correct Pattern
```bash
youtube-legend-cli --json "$url" 2>/dev/null | jaq '.body'
```

## Error Handling

### REQUIRED
- Branch on the `AppError` category in the envelope
- Map categories to retry policy in your orchestration layer
- Read `docs/AGENTS.md` for the full category table
- The `AppError` enum is `#[non_exhaustive]`; treat every variant as a category
- Use the `reason()` helper to extract `NoSubtitleReason` when the error is `NoSubtitle`

### FORBIDDEN
- Do not panic in pipeline logic on non-zero exit
- Do not assume a single error category for the whole provider chain
- Do not stringify the error to match on substrings

### Correct Pattern
```rust
match err {
    AppError::NoSubtitle(reason) => log::warn!("no subtitle: {reason}"),
    AppError::RateLimited { retry_after_secs } => {
        tokio::time::sleep(Duration::from_secs(retry_after_secs.unwrap_or(60))).await;
    }
    _ => return Err(err),
}
```

## Environment Variables

### REQUIRED
- `YT_LOG_LEVEL` wins over `--log-level`
- `YT_LOG_FORMAT` wins over `--log-format`
- `YT_LEGEND_CACHE_DIR` overrides the default XDG cache directory
- `YT_LEGEND_NO_NETWORK` disables all network traffic for offline mode
- Use the `YT_*` family for any configuration override

### FORBIDDEN
- Do not set `RUST_LOG` directly
- Do not pass log flags and env vars that conflict
- Do not rely on `RUST_LOG` to win over `YT_*` env vars

### Correct Pattern
```bash
YT_LOG_LEVEL=debug YT_LOG_FORMAT=json youtube-legend-cli "$url"
```

## YouTube Direct Provider Capabilities (v0.3.0)

### REQUIRED
- Pass `--provider youtube-direct` to force the native provider
- Trust auto-generated captions via `--asr` without fallback to third-party
- Receive canonical SRT from YouTube converted from Srv3 and Json3 locally
- Diagnose with `youtube-direct-probe <video-id>` when the decipher fails
- Apply filters on `captionTracks` by `languageCode` and `kind`

### FORBIDDEN
- Do not invoke `youtube-direct-probe` in production pipelines
- Do not assume the manual track exists before trying the auto-generated one

### Correct Pattern
```bash
youtube-direct-probe <video-id> | jaq -r '.signature_status'
```

## Diagnostic Binary `youtube-direct-probe`

### REQUIRED
- The binary lives at `src/bin/youtube-direct-probe.rs` and compiles alongside the CLI
- The probe loads the cached `base.js` and runs the decipher on a synthetic signature
- The probe prints one JSON object per line
- Each object carries `signature_status`, `player_js_version`, `cache_hit`, and optional `decipher_error`
- The probe honours `YT_LEGEND_NO_NETWORK` for offline diagnostics

### FORBIDDEN
- Do not invoke the probe in production loops
- Do not parse the output as if it were the subtitle body

### Correct Pattern
```bash
youtube-direct-probe dQw4w9WgXcQ
{"signature_status":"ok","player_js_version":"vfl123","cache_hit":true}
```

## Error Behaviour (v0.3.0)

### REQUIRED
- `SignatureDecipherFailed(String)` returns exit 70 `EX_SOFTWARE`
- `PlayerResponseMissing(String)` returns exit 70
- `CaptionTrackNotFound` returns exit 70
- `TimedtextUpstreamError(String)` returns exit 70
- The four variants live in `src/error.rs` and are additive to the `AppError` enum
- Library clients keep working without recompilation

### FORBIDDEN
- Do not treat the new variants as `EX_UNAVAILABLE`
- Do not confuse `SignatureDecipherFailed` with rate limiting

### Correct Pattern
```rust
match err {
    AppError::SignatureDecipherFailed(s) => log::error!("decipher failed: {s}"),
    AppError::PlayerResponseMissing(s) => log::error!("player response missing: {s}"),
    AppError::CaptionTrackNotFound => log::warn!("no caption track"),
    AppError::TimedtextUpstreamError(s) => log::error!("upstream timedtext failed: {s}"),
    _ => return Err(err),
}
```

## GAP-001 Milestones (M1 to M5 plus M3.5)

### REQUIRED
- M1 implements the `ytInitialPlayerResponse` parser in `src/provider/youtube/player_response.rs`
- M2 implements the signature-less timedtext fetcher using direct `baseUrl`
- M3 implements the signature decipher ported from `base.js` with XDG cache
- M3.5 implements the n-parameter decipher for protected videos in `src/provider/youtube/ncode.rs`
- M4 integrates the provider into the chain and adds `--provider`, `--asr`, and `--no-fallback` flags
- M5 adds test fixtures under `tests/fixtures/player/` and `tests/fixtures/timedtext/`
- The CI gate in `.github/workflows/youtube-direct.yml` requires all six cross-compile targets green

### FORBIDDEN
- Do not skip the YouTube direct CI gate before merge
- Do not change the Srv3 parser without updating the test snapshots

### Correct Pattern
```bash
# Run the YouTube direct provider tests
cargo test --test youtube_direct -- --ignored
```

## Cross-Compile Targets

### REQUIRED
- Six targets via the `cross-compile` job in `ci.yml`
- `x86_64-unknown-linux-gnu` is the primary development target
- `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` support static containers
- `x86_64-pc-windows-msvc` covers native Windows
- `x86_64-apple-darwin` and `aarch64-apple-darwin` run with `continue-on-error: true` because they require `osxcross`

### FORBIDDEN
- Do not ship a release without all six targets green in CI
- Do not trust local `cargo build` as a substitute for the cross-compile gate

### Correct Pattern
```bash
cargo install cross --locked
cross build --target x86_64-unknown-linux-musl --release
```
## See Also
- [CHANGELOG.md](../../CHANGELOG.md) — full release history
- [docs/AGENTS.md](../../docs/AGENTS.md) — agent guide with variant table
- [docs/MIGRATION.md](../../docs/MIGRATION.md) — v0.2.x to v0.3.x upgrade notes
- [docs/COOKBOOK.md](../../docs/COOKBOOK.md) — practical recipes for shell, CI, and Python
- [docs/ARCHITECTURE.md](../../docs/ARCHITECTURE.md) — pipeline diagram and module map
- [docs/CROSS_PLATFORM.md](../../docs/CROSS_PLATFORM.md) — cross-compile recipes and XDG paths
- [docs/TESTING.md](../../docs/TESTING.md) — integration test suite
- [docs/schemas/caption-track.schema.json](../../docs/schemas/caption-track.schema.json) — authoritative JSON schema
- [gaps.md](../../gaps.md) — living register of known issues
