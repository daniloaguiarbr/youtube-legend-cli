[English](AGENTS.md) | [Português Brasileiro](AGENTS.pt-BR.md)
# AGENTS

> A native-Unix subtitle fetcher that gives agents full control of the byte stream.

Languages: [English](docs/AGENTS.md) | [Português Brasileiro](docs/AGENTS.pt-BR.md)

## Why

- You are an agent, not a human. The CLI returns one URL in, one subtitle out, on plain `stdin` / `stdout`. No prompts, no TUI, no daemon to babysit.
- You already speak JSON. Pass `--json` and the CLI hands you a typed envelope with `provider`, `video_id`, `language`, `format`, `byte_size`, `source_url`, and the body. No string parsing, no fragile regex over free text.
- You already speak exit codes. The CLI returns BSD `sysexits.h` numbers so POSIX pipelines, `set -e` scripts, and your error handlers can branch on category without custom mapping.

## Economy

- A single subtitle download is roughly 60 percent smaller in tokens than scraping the watch page HTML and pulling captions by hand. The provider chain returns the timedtext payload directly, in the language you asked for, in the format you asked for.
- The local file cache at `~/.cache/youtube-legend-cli/` keys on `(video_id, language, format)`. Repeated requests for the same video in the same session are served from disk in microseconds.
- The JSON envelope is one line. Your parser reads it once, your LLM context holds the body, your prompt fits in the window.

## Sovereignty

- The binary is a single static Rust artifact. No runtime dependencies, no container, no cloud, no daemon, no background process. Drop it on a host and run it.
- Zero telemetry. The CLI never phones home, never sends analytics, never checks for updates. The only outbound traffic is the HTTP request to the chosen provider, scoped to the video you asked for.
- The `secret_endpoints` module is `pub(crate)` and gitignored. Upstream hostnames, cookie paths, and signing tokens never enter the published rustdoc or the public-api baseline.
- `SIGINT` and `SIGTERM` are cooperative. The first signal cancels in-flight work at the next `await` point and exits with code `130`. The second signal forces immediate process exit.

## Compatible Agents

- Claude Code — pipe a URL on `stdin`, capture the JSON envelope on `stdout`, branch on the exit code. Works in a `Bash` tool, works in a `cron` job, works in a hook.
- Aider — call the CLI from a shell command block, parse `--json` output, feed the body back into the next edit.
- Codex CLI — spawn the binary as a subprocess, read `stdout`, treat `stderr` as diagnostic only.
- Cline — use the CLI as a tool, pass a URL, capture the result, never let it touch the watch page HTML.
- Any LLM agent with a `bash` tool. The interface is plain Unix.

## Architecture at a Glance

A single clap-derived `Cli` struct captures the 17 flags. `commands::run` dispatches to `extract::run` for a single URL or to `batch::run` for stdin-driven lists. The provider chain walks `youtube-direct`, then `provider_a`, then `provider_b` (and `provider_headless` when the `headless` feature is enabled), throttled to one request per second, wrapped in `retry::retry_with_backoff` with three attempts at 1 s, 2 s, 4 s. The cache layer stores every successful fetch on disk. The output layer writes plain text, SRT, or the JSON envelope to `stdout`; logs and progress go to `stderr`.


## CLI Flags

### REQUIRED

- Use `--json` whenever a downstream consumer parses the output. Plain text output is for humans and pipes that do not care about structure.
- Pass `--lang` with a BCP 47 tag (`pt-BR`, `en-US`, `pt_BR.UTF-8`) when you need a specific language. The default `en` is a guess.
- Set `--timeout` in seconds for network-bound flows. The default of 30 seconds is appropriate for interactive use; longer pipelines should raise it.

### FORBIDDEN

- Do not hardcode the provider hostname in agent code. The provider chain is the public contract; the hostnames are gitignored and may change without notice.
- Do not parse `stderr`. Logs are human-readable and may include tracing spans; the structured data lives on `stdout` when `--json` is set.
- Do not use the `headless` provider without the `headless` feature flag. The binary refuses to invoke Chromium without the feature gate at build time.

### Correct Pattern

```bash
youtube-legend-cli --json "https://youtu.be/dQw4w9WgXcQ" \
  | jq -r '.body'
```

Discover every flag with `youtube-legend-cli --help`. The full table lives in the project `README.md`.


## Headless Mode (v0.3.1+)

### Problem
- The plain HTTP providers (`youtube-direct`, `provider-a`,
  `provider-b`) receive HTTP 400 from YouTube and HTTP 429 from
  Cloudflare when called from a datacenter IP.
- Downstream operators need a path that drives a real browser so
  Cloudflare can resolve the JavaScript challenge.

### Correct Pattern
- Install the binary with the `headless` feature:
  `cargo install youtube-legend-cli --version 0.3.1 --features headless`
- Invoke with `--headless` to force the headless fallback:
  `youtube-legend-cli --headless "https://youtu.be/dQw4w9WgXcQ"`
- Set `$CHROME=/path/to/chrome` to override the executable lookup
- `BrowserFetcher` auto-downloads a portable Chromium build to
  `$XDG_CACHE_HOME/youtube-legend-cli/browser/` when no local Chrome
  is found
- `$YT_LEGEND_NO_NETWORK=1` short-circuits the provider without
  spawning a browser (returns `AppError::ProviderUnavailable`)

### Exit Codes for Headless Failures
- 66 (EX_NOINPUT) — no matching language track after Cloudflare
  challenge resolved
- 69 (EX_UNAVAILABLE) — Chrome/Chromium missing AND auto-download
  failed, OR `$YT_LEGEND_NO_NETWORK` is set
- 70 (EX_SOFTWARE) — 60-second navigation timeout exceeded

## JSON Envelope

### REQUIRED

- Parse `provider`, `video_id`, `language`, `format`, `byte_size`, `source_url` as typed fields. Do not extract them with regex.
- Branch on the `error` field when it is non-null. The envelope is the source of truth for failure modes.

### FORBIDDEN

- Do not ignore the `error` field. Every non-success envelope carries a structured failure with a `kind` and a human message.
- Do not assume the body is a string. It is a UTF-8 string when `--format txt` is set and a byte-for-byte SRT payload when `--format srt` is set.

### Correct Pattern

```json
{
  "provider": "youtube-direct",
  "video_id": "dQw4w9WgXcQ",
  "language": "en",
  "format": "txt",
  "byte_size": 1452,
  "source_url": "https://www.youtube.com/api/timedtext...",
  "body": "...",
  "error": null
}
```


## Provider Chain

### REQUIRED

- Let the chain auto-fallback. The `auto` policy tries `youtube-direct` first, then `provider_a`, then `provider_b` (then `provider_headless` if the binary was built with the feature).
- Honor `--asr` when the requester wants the auto-generated track even when a manual track exists.

### FORBIDDEN

- Do not pin a single provider in production CI. The whole point of the chain is graceful degradation when one upstream is degraded.
- Do not combine `--asr` with `provider_a` or `provider_b`. The third-party providers do not expose a manual-versus-ASR selection; the CLI rejects the combination with exit code `64`.

### Correct Pattern

```bash
# Production: let the chain auto-fallback
youtube-legend-cli --provider auto "https://youtu.be/VIDEO"

# Debug: pin a provider and disable fallback
youtube-legend-cli --provider youtube-direct --no-fallback "https://youtu.be/VIDEO"
```


## Exit Codes

### REQUIRED

- Branch on the BSD `sysexits.h` category. `0` is success, `64` is usage error, `65` is data error, `66` is no input, `69` is unavailable, `70` is software error, `78` is configuration error, `130` is signal.
- Use `AppError::exit_code()` when consuming the Rust API directly. The mapping is the same table.

### FORBIDDEN

- Do not hardcode raw integers in CI scripts. Map them by category name in your shell dispatcher.
- Do not treat `69` as a fatal error. It means the upstream was unavailable; retry with backoff and a different provider.

### Correct Pattern

```bash
case "$(youtube-legend-cli --json ...; echo $?)" in
  0)   handle_success ;;
  64|65|78) handle_user_error ;;
  66)  handle_no_subtitle ;;
  69)  handle_upstream_unavailable ;;
  70)  handle_internal_error ;;
  130) handle_signal ;;
  *)   handle_unknown ;;
esac
```


## Cache

### REQUIRED

- Use the default 24-hour TTL. The cache layer at `~/.cache/youtube-legend-cli/` is keyed on `(video_id, language, format)` and is safe to share across runs.
- Use `--no-cache` for one-off reads that must reflect the current upstream state, not the cached snapshot.

### FORBIDDEN

- Do not redirect the cache to `/tmp`. The cache directory is created and managed by the `directories` crate; bypassing it loses the cross-run benefit.
- Do not hand-edit cache files. The format is internal and the next run will overwrite inconsistent entries.

### Correct Pattern

```bash
# Override the TTL for a long-running batch
youtube-legend-cli --cache-ttl 168 "https://youtu.be/VIDEO"

# Force a fresh read, bypass cache
youtube-legend-cli --no-cache "https://youtu.be/VIDEO"
```


## Retry and Rate Limiting

### REQUIRED

- Honor the `Retry-After` header in both delta-seconds and RFC 2822 HTTP-date form. The CLI already does this in `retry::retry_with_backoff`; the fallback is 60 seconds, capped at 300.
- Treat `AppError::RateLimited` as transient. The provider will recover; the chain will retry.

### FORBIDDEN

- Do not add a custom retry loop in agent code. The CLI has its own backoff and circuit breaker; nesting retries causes stampedes.
- Do not pin `--timeout` below 5 seconds. The first request is throttled to one per second; a tight timeout triggers spurious failures.

### Correct Pattern

```bash
# The CLI handles Retry-After internally
youtube-legend-cli "https://youtu.be/VIDEO"
## YouTube Direct Provider Capabilities (v0.3.0)

Agents consuming the CLI can now:

- Pass `--provider youtube-direct` to force the native provider.
- Trust auto-generated captions (ASR) without fallback to third-party.
- Receive canonical SRT from YouTube (Srv3/Json3 converted locally).
- Diagnose with `youtube-direct-probe <video-id>` (probe binary).

## Error Behaviour (v0.3.0)

New variants in `AppError`:

- `SignatureDecipherFailed(String)`: exit 70 (`EX_SOFTWARE`).
- `PlayerResponseMissing(String)`: exit 70.
- `CaptionTrackNotFound`: exit 70.
- `TimedtextUpstreamError(String)`: exit 70.


# Inspect the parsed Retry-After when consuming the Rust API
match err {
    AppError::RateLimited { retry_after_secs } => sleep(Duration::from_secs(retry_after_secs.unwrap_or(60))),
    _ => return Err(err),
}
```


## Streaming Contracts

### REQUIRED

- Treat `stdout` as the subtitle body, or the JSON envelope when `--json` is set. The contract is exclusive.
- Treat `stderr` as logs, progress, and human error messages. Capture for debugging; never parse.

### FORBIDDEN

- Do not write your own logs to `stdout`. The byte stream downstream is the subtitle; any non-subtitle data corrupts the output.
- Do not redirect `stderr` to `/dev/null` in CI. You will lose the failure reason when the exit code is non-zero.

### Correct Pattern

```bash
# Capture the body and the logs separately
youtube-legend-cli "https://youtu.be/VIDEO" > subtitle.txt 2> run.log
```


## Error Handling

### REQUIRED

- Map `AppError` to a category. The enum is `#[non_exhaustive]`; treat every variant as a category, not a specific case.
- Use the `reason()` helper to extract the inner `NoSubtitleReason` when the error is `NoSubtitle`. The default branch returns `NotPublished`.

### FORBIDDEN

- Do not `panic!` on `AppError`. The library API is total; every failure has a typed variant and a human-readable message.
- Do not stringify the error to match on substrings. The typed variants are the contract; substrings are unstable.

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

- Set `YT_LOG_LEVEL` to one of `error`, `warn`, `info`, `debug`, `trace` to override `--log-level` at runtime. The CLI honors the env var above the CLI value.
- Set `YT_LOG_FORMAT=json` in production for machine-parseable logs. The CLI writes to `stderr` only.

### FORBIDDEN

- Do not read the environment directly from agent code. The CLI consumes `YT_LOG_LEVEL` and `YT_LOG_FORMAT`; let the binary do it.
- Do not set `RUST_LOG` and expect it to win. The CLI uses an `EnvFilter` that prefers `YT_LOG_LEVEL` over `RUST_LOG`.

### Correct Pattern

```bash
# Production: structured JSON logs at info level
export YT_LOG_LEVEL=info
export YT_LOG_FORMAT=json
youtube-legend-cli "https://youtu.be/VIDEO"
```
