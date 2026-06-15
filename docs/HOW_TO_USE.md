[English](HOW_TO_USE.md) | [Português Brasileiro](HOW_TO_USE.pt-BR.md)
# How to Use — youtube-legend-cli

> Run one command, get a clean subtitle file. No daemon, no prompts, no telemetry.

[English](docs/HOW_TO_USE.md) | [Português Brasileiro](docs/HOW_TO_USE.pt-BR.md)

This page is the practical 60-second walkthrough. It assumes you
have a YouTube URL and want the subtitle text on your terminal. The
flag reference, exit codes, and provider chain live in
[`README.md`](README.md) and
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). The integration
surface for AI agents and CI is in
[`INTEGRATIONS.md`](INTEGRATIONS.md).

## Prerequisites

- A Unix-like shell on Linux, macOS, or Windows 10/11.
- Rust 1.88.0 or newer if you plan to build from source.
- `curl` NÃO é necessário. The CLI talks HTTP itself.
- No Python, no Node, no system services.

## First Command in 60 Seconds

The CLI ships as a single static binary, so the three steps below
take less than a minute on a warm connection.

### 1. Install

Install from crates.io for the cleanest path.

```bash
cargo install youtube-legend-cli
```

Or build from a local checkout.

```bash
cargo install --path .
```

### 2. Run

Pipe a YouTube URL into the binary and redirect `stdout` to a file.
The body of the subtitle lands in `subtitle.txt`; logs and progress
stay on `stderr`.

```bash
youtube-legend-cli "https://youtu.be/NvZ4VZ5hooY" > subtitle.txt
```

### 3. Verify

Spot-check the body length and the first non-empty line.

```bash
wc -c subtitle.txt
head -n 3 subtitle.txt
```

## Core Commands
## ProviderYouTubeDirect (v0.3.0)

Starting at v0.3.0, the CLI queries YouTube directly as a primary
provider, with no third-party service in the loop.

### How It Works

- Issues a `GET` to `https://www.youtube.com/watch?v=<id>` with
  realistic headers.
- Extracts `ytInitialPlayerResponse` from the returned HTML.
- Walks `captions.playerCaptionsTracklistRenderer.captionTracks[]`.
- For each track, resolves `baseUrl` (with decipher when a signature
  is present).
- Downloads the Srv3/Json3 payload and converts to SRT locally.

### When To Use

- Videos whose auto-generated captions (ASR) third-party providers
  do not index.
- Pipelines that need a consistent hit rate.
- Environments where depending on external services is undesirable.

The CLI follows a single convention: the subtitle body goes to
`stdout`, every other diagnostic goes to `stderr`, and `stdin`
accepts either a single URL or a batch. The five commands below
cover the everyday cases.

### Single URL

```bash
youtube-legend-cli "https://youtu.be/dQw4w9WgXcQ" > subtitle.txt
```

### Batch From a File

One URL per line on `stdin` plus the `--batch` flag. The CLI reads
them sequentially and writes each subtitle body in order.

```bash
youtube-legend-cli --batch < urls.txt > subtitles.txt
```

### JSON Envelope

Pass `--json` to swap the body for a structured envelope. The
envelope is the contract downstream agents and CI jobs rely on.

```bash
youtube-legend-cli --json "https://youtu.be/NvZ4VZ5hooY"
```

### Custom Language

Use any BCP 47 tag, including the underscored forms that some
YouTube captions publish.

```bash
youtube-legend-cli --lang pt-BR "https://youtu.be/dQw4w9WgXcQ"
youtube-legend-cli --lang pt_BR.UTF-8 "https://youtu.be/dQw4w9WgXcQ"
```

### Custom Format

Switch from plain text to preserved SRT when you need the original
timing.

```bash
youtube-legend-cli --format srt "https://youtu.be/dQw4w9WgXcQ" > subtitle.srt
```

## Configuration

The `--config` flag points the CLI at a TOML file with defaults
that the command line would otherwise have to repeat. A typical
config file pins the language, the format, and the cache TTL.

```toml
# yt-legend.toml
lang = "pt-BR"
format = "srt"
cache_ttl = 48
verbose = false
```

Pass the file path to the CLI to apply the defaults.

```bash
youtube-legend-cli --config ./yt-legend.toml \
  "https://youtu.be/NvZ4VZ5hooY"
```

Command-line flags override the file. A flag absent from the file
keeps its built-in default.

## Integration With AI Agents

The CLI is designed to be spawned as a subprocess. The examples
below show the three patterns that come up most often in agent
transcripts.

### Pattern One — Pipe JSON Into jq

The `--json` envelope is a stable contract. An agent that can spawn
a subprocess and read two streams can drive the entire workflow.

```bash
youtube-legend-cli --json "https://youtu.be/NvZ4VZ5hooY" \
  | jq -r '.body'
```

### Pattern Two — Quiet Batch Capture

`--quiet` keeps the agent transcript clean while the body still
arrives on `stdout`. `--batch` reads one URL per line.

```bash
printf '%s\n' \
  "https://youtu.be/NvZ4VZ5hooY" \
  "https://youtu.be/dQw4w9WgXcQ" \
  | youtube-legend-cli --quiet --batch
```

### Pattern Three — Dry Run From Cache

`--dry-run` skips network I/O and serves only from the local cache.
This is the safety net an agent should reach for when the same URL
has already been resolved in the same session.

```bash
youtube-legend-cli --dry-run --lang pt \
  "https://youtu.be/NvZ4VZ5hooY"
```

## Troubleshooting FAQ

### Why does the CLI exit with code 66?

Exit code 66 (`EX_NOINPUT`) means no subtitle track exists for the
requested language on the video. Try a different language, or run
with `--verbose` to confirm which track the upstream returned.

### Why does the CLI exit with code 69 on a known video?

Exit code 69 (`EX_UNAVAILABLE`) means the chain tried every
provider and every one returned a non-recoverable failure. The
common causes are rate limiting (`HTTP 429` with an exhausted
`Retry-After`), a `robots.txt` `Disallow`, or an upstream outage.
Wait a few minutes, then retry with `--no-cache` to bypass any
stale negative cache.

### How do I bypass a single bad provider?

Starting in v0.3.0, the `--provider` flag pins a specific provider
and `--no-fallback` disables the rest of the chain. The combination
isolates a misbehaving upstream.

```bash
youtube-legend-cli --provider provider_a --no-fallback \
  "https://youtu.be/dQw4w9WgXcQ"
```

### Where is the cache stored?

The local cache lives at `~/.cache/youtube-legend-cli/`, keyed on
`(video_id, language, format)`. The default TTL is 24 hours. Clear
it with `rm -rf ~/.cache/youtube-legend-cli/` if you need a cold
fetch.

### How do I log to JSON in CI?

Set `YT_LOG_FORMAT=json` in the environment. The
`tracing-subscriber` initialiser reads `EnvFilter` first, so the
env var wins over the `--log-format` flag.

```bash
YT_LOG_FORMAT=json youtube-legend-cli --json \
  "https://youtu.be/NvZ4VZ5hooY"
```
