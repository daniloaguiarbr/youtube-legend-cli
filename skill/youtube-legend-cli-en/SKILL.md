---
name: youtube-legend-cli
description: Triggers when the user asks to download YouTube subtitles, captions, transcripts, or text tracks from a YouTube URL (watch, shorts, embed, youtu.be). Also triggers on mentions of yt-dlp replacement, daniloaguiarbr, youtube-legend-cli, downsub alternative, noteey, save subs, or batch subtitle download from a list of YouTube URLs. Use to invoke the youtube-legend-cli Rust CLI for non-interactive, scriptable, JSON-enveloped subtitle retrieval via the noteey.com headless-browser provider. Covers CLI flags, the JSON envelope, BSD sysexits exit codes, caching, retry handling, the provider-noteey headless Chromium pipeline (BrowserFetcher auto-download, stealth fingerprint patches, $CHROME override), environment variables, and offline mode.
---


# youtube-legend-cli (v0.3.2)


## Identity and Architecture
- youtube-legend-cli is a non-interactive Rust CLI that downloads YouTube subtitles
- The CLI uses EXACTLY ONE provider: `provider-noteey` (noteey.com via headless Chromium)
- v0.3.2 REMOVED `youtube-direct`, `provider-a`, `provider-b`, and `provider-headless`
- v0.3.2 REMOVED the `--asr`, `--no-fallback`, and `--headless` flags
- The provider drives a headless Chromium instance through `chromiumoxide` 0.9.1
- The provider fills the noteey.com form, clicks "Get Subtitle", and polls the transcript pane
- noteey returns plain text with `MM:SS` (or `HH:MM:SS`) prefixes, NOT SubRip
- stdout carries ONLY the subtitle body or the JSON envelope
- stderr carries ONLY logs, progress bars, and diagnostics
- ALWAYS discard stderr before piping stdout to `jaq`


## Installation and First Run

### REQUIRED
- Rust toolchain 1.88 or newer on the host
- Build with the `headless` Cargo feature because provider-noteey depends on Chromium at runtime
- Allow outbound network on the first run so `BrowserFetcher` can reach Google Cloud Storage
- On first run `BrowserFetcher` auto-downloads Chromium revision `r1585606`
- The browser cache lives at `~/.cache/youtube-legend-cli/browser/`

### FORBIDDEN
- Do not expect a system Chromium to be used by default; the pinned revision is preferred
- Do not run the CLI in a tight loop in production scripts
- Do not install a local fork without auditing the diff first

### Correct Pattern
```bash
cargo install youtube-legend-cli
youtube-legend-cli "https://youtu.be/NvZ4VZ5hooY" > out.txt
```

## CLI Flags Reference

### REQUIRED
- Pass `--json` for every programmatic consumer
- `--lang` accepts ISO 639-1 codes or full BCP 47 locales: `en`, `pt`, `pt-BR`, `es`, `fr`, `de`, `it`
- `--lang` normalizes to the primary subtag, so `pt-BR`, `pt_BR.UTF-8`, and `EN-us` all work
- `--format` accepts `txt` (timestamps stripped) or `srt` (raw text preserved)
- `--provider` accepts ONLY `auto` (default) or `provider-noteey`; both resolve to noteey.com
- `--timeout` accepts positive integer seconds for the HTTP bound (default 30)
- `--config <PATH>` loads a flat TOML configuration file
- `--cache-ttl` accepts positive integer hours to override the TTL (default 24)
- `--no-cache` forces a fresh read ignoring the local cache
- `--no-progress` suppresses progress bars on stderr
- `--dry-run` skips network I/O and serves reads from cache only
- `--yes` assumes yes for non-interactive prompts
- `--batch` reads URLs from stdin, one per line
- `--user-agent` overrides the HTTP User-Agent header
- `--verbose` and `--quiet` control log volume on stderr; they are mutually exclusive
- `--log-level` accepts `error`, `warn`, `info`, `debug`, `trace`
- `--log-format` accepts `text` or `json`
- `--color` accepts `auto`, `always`, `never`
- Combine `--json` with `--lang` for localized output envelopes

### FORBIDDEN
- Do not combine `--batch` with a positional URL — exit 64
- Do not combine `--quiet` with `--verbose` — exit 64
- Do not combine `--dry-run` with `--batch` — exit 64
- Do not pass `--timeout 0` or `--cache-ttl 0` — exit 64
- Do not pass a URL longer than 2048 characters — exit 64
- Do not pass a YouTube URL as a positional argument AND through stdin
- Do not hardcode hostnames in scripts

### Correct Pattern
```bash
youtube-legend-cli --json --lang pt-BR "https://youtu.be/abc" | jaq '.body'
```

## Standard Invocation Patterns

### Correct Pattern
```bash
# Single URL to plain text
youtube-legend-cli https://youtu.be/dQw4w9WgXcQ

# Single URL through stdin, raw text preserved
echo "https://youtu.be/dQw4w9WgXcQ" | youtube-legend-cli --format srt

# JSON envelope, Brazilian Portuguese, 60-second timeout
youtube-legend-cli --json --lang pt --timeout 60 https://youtu.be/dQw4w9WgXcQ | jaq '.body'

# Batch from a file, JSON per line
cat urls.txt | youtube-legend-cli --batch --json

# Force the provider explicitly for deterministic tests
youtube-legend-cli --provider provider-noteey https://youtu.be/dQw4w9WgXcQ
```

## JSON Envelope and Schema

### REQUIRED
- Validate the `error` field on stdout before trusting the body
- Branch on the `code` field matching BSD sysexits
- Read the `retry_after_seconds` field when present
- Pipe stdout through `jaq` or an equivalent JSON parser
- The envelope fields are `provider`, `video_id`, `language`, `format`, `byte_size`, `source_url`, `body`, `error`
- The `provider` field is always `provider-noteey`

### FORBIDDEN
- Do not parse stdout line-by-line as raw subtitle text when `--json` is set
- Do not skip the envelope check
- Do not assume the body is always a string

### Correct Pattern
```bash
out=$(youtube-legend-cli --json "$url" 2>/dev/null)
echo "$out" | jaq -e '.error == null' >/dev/null && echo "$out" | jaq -r '.body' || echo "$out" | jaq -r '.error'
```

## Exit Codes and sysexits.h

### REQUIRED
- `0` for success
- `2` for a clap parser rejection (invalid flag value, e.g. `--lang xx`)
- `64` EX_USAGE on invalid flag combinations and invalid input
- `65` EX_DATAERR on a malformed or unrecognized YouTube URL
- `66` EX_NOINPUT when the video has no matching subtitle
- `69` EX_UNAVAILABLE when the provider is unavailable, rate-limited, Chromium is missing, or a captcha appears
- `70` EX_SOFTWARE on internal failure, timeout, HTTP, I/O, crypto, and parse errors
- `78` EX_CONFIG on a malformed or unreadable config file
- `130` SIGINT or SIGTERM on user interrupt

### FORBIDDEN
- Do not rely on exact exit numbers without the category mapping
- Do not mask the exit code with a `|| true` fallback

### Correct Pattern
```bash
youtube-legend-cli "$url" || case $? in
  64) echo "usage error" ;;
  65) echo "bad url" ;;
  66) echo "no subtitles" ;;
  69) echo "provider unavailable or chromium missing" ;;
  70) echo "internal failure" ;;
  78) echo "config error" ;;
  *) echo "other failure" ;;
esac
```

## Provider noteey (Headless Chromium)

### REQUIRED
- The exclusive provider since v0.3.2 is `provider-noteey`
- The provider drives noteey.com through a headless Chromium instance via `chromiumoxide` 0.9.1
- The provider navigates to noteey.com, fills the URL input, clicks "Get Subtitle", and polls the transcript pane for up to 30 seconds
- noteey returns plain text with `MM:SS` timestamp prefixes, NOT SubRip
- `--format txt` strips the prefixes
- The provider applies anti-fingerprint stealth patches before navigation
- The provider respects `$YT_LEGEND_NO_NETWORK` and short-circuits to `ProviderUnavailable` (exit 69)

### FORBIDDEN
- Do not expect `--format srt` to synthesise real SubRip timestamps; noteey returns plain text
- Do not expect a captcha to resolve by retrying; a captcha returns exit 69

### Correct Pattern
```bash
youtube-legend-cli --provider provider-noteey --lang pt-BR https://youtu.be/VIDEO > legenda.txt
```

## BrowserFetcher and Chromium Resolution

### REQUIRED
- The provider resolves a Chromium executable in a STRICT order
- Order 1: the `$CHROME` environment variable (operator override, highest priority)
- Order 2: `BrowserFetcher` auto-download of the pinned revision `r1585606` into `~/.cache/youtube-legend-cli/browser/`
- Order 3: well-known system paths as a last-resort fallback (`/usr/bin/chromium-browser`, `/usr/bin/chromium`, `/usr/bin/google-chrome`, `/usr/bin/google-chrome-stable`, `/usr/bin/brave-browser`)
- Prefer the BrowserFetcher revision over a system browser
- A system Chromium build is frequently newer than the CDP protocol `chromiumoxide` 0.9.1 targets, which makes `Browser::launch` fail with a protocol reset
- Set `$CHROME` to pin a known-compatible binary and skip the download
- The chrome profile lives at `~/.cache/youtube-legend-cli/chrome-profile/`
- The provider sweeps stale Chrome singleton lock files before each launch

### FORBIDDEN
- Do not assume the download bundles into the binary; Chromium downloads at runtime
- Do not delete the entire cache directory in production scripts

### Correct Pattern
```bash
# Pin a compatible binary and skip the download
CHROME=/usr/bin/chromium youtube-legend-cli https://youtu.be/VIDEO

# Recover from a corrupted profile
rm -rf ~/.cache/youtube-legend-cli/chrome-profile/
```

## Stealth Anti-Fingerprint Patches

### REQUIRED
- The provider injects anti-fingerprint JavaScript via CDP `Page.addScriptToEvaluateOnNewDocument`
- The patches run in every new document BEFORE any other JavaScript
- Patch 1 sets `navigator.webdriver` to `undefined`
- Patch 2 injects a 3-entry `navigator.plugins` array (`Chrome PDF Plugin`, `Chrome PDF Viewer`, `Native Client`)
- Patch 3 overrides `navigator.languages` with `["pt-BR", "en-US", "en"]`
- Patch 4 overrides the WebGL vendor and renderer to mask the SwiftShader software rasterizer (`Intel Inc.` / `Intel Iris OpenGL Engine`)
- Patch 5 installs a minimal `window.chrome.runtime` mock so `chrome?.` probing code does not throw
- The patches are applied AFTER `new_page` and BEFORE the first `goto`

### FORBIDDEN
- Do not expect these patches to defeat a real captcha; a `CaptchaChallenge` returns exit 69
- Do not edit the patch set without updating the `stealth.rs` regression tests

## Cache Behaviour

### REQUIRED
- 24-hour TTL on disk at `~/.cache/youtube-legend-cli/`
- The browser cache lives at `~/.cache/youtube-legend-cli/browser/`
- The chrome profile lives at `~/.cache/youtube-legend-cli/chrome-profile/`
- Use `--no-cache` for fresh fetches in audit pipelines
- Use `--cache-ttl` to override the TTL in positive integer hours
- Invalidate a single entry by removing its directory
- Override the cache root with `$YT_LEGEND_CACHE_DIR`

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
- The one-request-per-second throttle is per run, not per provider

### FORBIDDEN
- Do not run client-side retry loops without backoff
- Do not set `--timeout` below 5 seconds

### Correct Pattern
```bash
out=$(youtube-legend-cli --json "$url" 2>/dev/null)
delay=$(echo "$out" | jaq -r '.retry_after_seconds // empty')
[ -n "$delay" ] && sleep "$delay"
```

## Streaming Contracts

### REQUIRED
- stdout carries subtitle text or the JSON envelope only
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
- The `AppError` enum is `#[non_exhaustive]`; treat every variant as a category
- Use the `reason()` helper to extract `NoSubtitleReason` when the error is `NoSubtitle`
- Use `is_captcha()` to distinguish a captcha challenge from a transient `ProviderUnavailable`
- A `BrowserNotFound` error means Chromium could not be resolved; install a browser or set `$CHROME`
- A `CaptchaChallenge` requires human interaction and CANNOT resolve by retrying

### FORBIDDEN
- Do not panic in pipeline logic on a non-zero exit
- Do not stringify the error to match on substrings
- Do not assume a single error category for the provider

### Correct Pattern
```rust
match err {
    AppError::NoSubtitle(reason) => log::warn!("no subtitle: {reason}"),
    AppError::RateLimited { retry_after_secs } => {
        tokio::time::sleep(Duration::from_secs(retry_after_secs.unwrap_or(60))).await;
    }
    AppError::BrowserNotFound(s) => log::error!("chromium not found: {s}"),
    AppError::CaptchaChallenge { provider, kind } => {
        log::error!("captcha from {provider} ({kind}); human interaction required")
    }
    AppError::ProviderUnavailable => log::warn!("provider unavailable"),
    _ => return Err(err),
}
```

## Environment Variables

### REQUIRED
- `YT_LOG_LEVEL` wins over `--log-level`
- `YT_LOG_FORMAT` wins over `--log-format`
- `YT_LEGEND_CACHE_DIR` overrides the default XDG cache directory
- `YT_LEGEND_NO_NETWORK` disables all network I/O; the provider returns `ProviderUnavailable` (exit 69)
- `CHROME` pins the Chromium/Chrome executable and skips the BrowserFetcher download
- `RUST_LOG` is honoured ONLY when `--log-level` is at its default
- `NO_COLOR` and `CLICOLOR_FORCE` are honoured when `--color` is `auto`
- Use the `YT_*` family for any configuration override

### FORBIDDEN
- Do not set `RUST_LOG` directly when a `YT_*` override applies
- Do not pass log flags and env vars that conflict
- Do not rely on `RUST_LOG` to win over the `YT_*` env vars

### Correct Pattern
```bash
YT_LOG_LEVEL=debug YT_LOG_FORMAT=json youtube-legend-cli "$url"

# Offline mode returns exit 69, ProviderUnavailable
YT_LEGEND_NO_NETWORK=1 youtube-legend-cli "$url"
```

## Config File (TOML)

### REQUIRED
- `--config <PATH>` loads a flat TOML table whose keys mirror the long flag names without the leading `--`
- Precedence is CLI flag, then config value, then built-in default
- An explicit CLI flag always wins, even when its value equals the default
- Supported keys: `url`, `lang`, `format`, `timeout`, `cache_ttl`, `user_agent`, `provider`
- Boolean keys: `verbose`, `quiet`, `json`, `batch`, `no_cache`, `dry_run`, `no_progress`, `yes`
- Optional keys: `log_level`, `log_format`, `color`

### FORBIDDEN
- Do not add an unknown key; an unknown key fails with exit 78
- Do not write malformed TOML; a parse error fails with exit 78

### Correct Pattern
```toml
lang = "pt"
format = "txt"
timeout = 60
provider = "provider-noteey"
```

## Cross-Compile and Build Notes

### REQUIRED
- The `headless` Cargo feature is REQUIRED for provider-noteey
- Build with `cargo build --release` for the host target
- Cross-compile with `cargo build --release --target <triple>` after installing the target toolchain
- Verify the target host can run the pinned Chromium revision or has `$CHROME` set
- Set `YT_LEGEND_NO_NETWORK=1` in CI without network so tests surface a clean exit 69 instead of timing out on a browser launch

### FORBIDDEN
- Do not assume cross-compiling the binary bundles Chromium; Chromium is a runtime dependency
- Do not trust a local `cargo build` as a substitute for the cross-compile gate

### Correct Pattern
```bash
cargo install cross --locked
cross build --target x86_64-unknown-linux-musl --release
```

## See Also
- [CHANGELOG.md](../../CHANGELOG.md) — full release history
- [docs/AGENTS.md](../../docs/AGENTS.md) — agent guide with the error category table
- [docs/schemas/caption-track.schema.json](../../docs/schemas/caption-track.schema.json) — authoritative JSON schema
- [gaps.md](../../gaps.md) — living register of known issues
