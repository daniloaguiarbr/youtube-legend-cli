[English](COOKBOOK.md) | [Português Brasileiro](COOKBOOK.pt-BR.md)
# COOKBOOK

> Practical recipes for driving the subtitle CLI from a shell, a CI runner, or a Python pipeline.

Languages: [English](docs/COOKBOOK.md) | [Português Brasileiro](docs/COOKBOOK.pt-BR.md)

## Latency Note

The cache layer lives at `~/.cache/youtube-legend-cli/`. On a warm cache the body is served from disk in roughly one millisecond. On a cold cache the latency is dominated by the upstream HTTP round-trip plus throttling: `youtube-direct` averages 800 ms to 1.5 s end-to-end on a typical residential connection, `provider_a` averages 1.5 s to 3 s, and `provider_b` averages 2 s to 4 s because of the AES-256-CBC plus PBKDF2 token signing round-trip. The one-request-per-second throttle is per chain, not per provider, so back-to-back downloads on a cold cache pay the throttle cost on every call.

## Default Values Reference

| Setting | Default | Flag |
|---|---|---|
| Language | `en` | `--lang` |
| Format | `txt` | `--format` |
| HTTP timeout | 30 s | `--timeout` |
| Verbose logging | off | `--verbose` |
| Suppress non-error logs | off | `--quiet` |
| JSON envelope | off | `--json` |
| Batch from stdin | off | `--batch` |
| User-Agent | crate name | `--user-agent` |
| Cache TTL | 24 hours | `--cache-ttl` |
| Skip cache reads | off | `--no-cache` |
| Log level | `warn` | `--log-level` |
| Log format | `text` | `--log-format` |
| Color | `auto` | `--color` |
| Progress bars | on | `--no-progress` |
| Dry run (cache only) | off | `--dry-run` |
| Assume yes for prompts | off | `--yes` |
| Provider | `auto` | `--provider` |
| Prefer ASR track | off | `--asr` |
| Disable fallback chain | off | `--no-fallback` |


## How To download subtitles for one video

PROBLEM: A user gives you a single YouTube URL and you need the plain-text transcript on disk.

SOLUTION: Pipe the URL on the command line, redirect `stdout` to a file. The body lands in the file; logs and progress land on the terminal.

```bash
youtube-legend-cli "https://youtu.be/dQw4w9WgXcQ" > subtitle.txt
```

VERIFY: The file exists, contains the transcript, and the terminal shows zero or more progress lines on `stderr` only.

```bash
wc -l subtitle.txt
head -n 3 subtitle.txt
```


## How To download in batch from a list

PROBLEM: You have a file with one URL per line and you need a transcript for every video.

SOLUTION: Pass `--batch` and pipe the file on `stdin`. Each line is processed in order; a non-fatal failure on one line does not abort the rest.

```bash
youtube-legend-cli --batch < urls.txt > transcripts.txt 2> batch.log
```

VERIFY: `transcripts.txt` contains every successful body concatenated in input order, separated by a header line, and `batch.log` shows the per-URL status.

```bash
grep -c "^=== " transcripts.txt
cat batch.log
```


## How To parse the JSON envelope in Python

PROBLEM: A pipeline needs the structured fields (`video_id`, `language`, `byte_size`, `body`) without writing a regex.

SOLUTION: Use `--json` to make the CLI emit a one-line JSON envelope, then parse it with `json.loads`.

```python
import json
import subprocess

result = subprocess.run(
    ["youtube-legend-cli", "--json", "https://youtu.be/dQw4w9WgXcQ"],
    capture_output=True,
    text=True,
    check=False,
)
envelope = json.loads(result.stdout)
if envelope.get("error"):
    raise SystemExit(f"provider error: {envelope['error']}")
print(envelope["body"])
```

VERIFY: The script prints the transcript to `stdout` and exits with code `0`. If the upstream is unavailable, `envelope["error"]` is a structured object and the script exits non-zero.


## How To switch providers for CI

PROBLEM: CI runs need a deterministic provider to avoid flakes when one upstream is degraded.

SOLUTION: Pass `--provider` to pin a single provider, and pass `--no-fallback` to disable the chain. The CLI exits with `69` if the pinned provider fails instead of trying the next one.

```bash
youtube-legend-cli --provider youtube-direct --no-fallback \
  "https://youtu.be/dQw4w9WgXcQ" > subtitle.txt
```

VERIFY: The exit code is `0` on success, `69` on upstream unavailability, never `0` from a different provider than the one you pinned.


## How To override cache TTL

PROBLEM: A long-running batch process needs a longer cache window so repeated downloads of the same video are free.

SOLUTION: Pass `--cache-ttl` in hours. The value is a positive integer; the cache layer applies it on every read.

```bash
youtube-legend-cli --cache-ttl 168 \
  "https://youtu.be/dQw4w9WgXcQ" > subtitle.txt
```

VERIFY: A second invocation of the same command on the same video completes in under 10 ms and produces no upstream network traffic.

```bash
time youtube-legend-cli --cache-ttl 168 "https://youtu.be/dQw4w9WgXcQ" > /dev/null
```


## How To handle HTTP 429 from upstream

PROBLEM: A provider answers with HTTP 429 and a `Retry-After` header. The pipeline needs to wait and try again.

SOLUTION: The CLI already honors `Retry-After` internally via `retry::retry_with_backoff`. From the outside, the only thing to do is to surface the structured error and wait.

```bash
output=$(youtube-legend-cli --json "https://youtu.be/VIDEO" 2>/dev/null)
if [ "$(echo "$output" | jq -r '.error.kind')" = "rate_limited" ]; then
  retry_after=$(echo "$output" | jq -r '.error.retry_after_secs // 60')
  echo "rate limited, sleeping ${retry_after}s" >&2
  sleep "$retry_after"
  youtube-legend-cli --json "https://youtu.be/VIDEO"
fi
```

VERIFY: The first command exits with code `69` and emits a JSON envelope with `error.kind = "rate_limited"`. The second command (after the sleep) succeeds with `error` set to `null`.


## How To debug with verbose logging

PROBLEM: A download fails and you need to see the provider chain, retry attempts, and HTTP timings.

SOLUTION: Combine `--verbose` with `--log-level debug` to get tracing events on `stderr` and keep the body clean on `stdout`.

```bash
youtube-legend-cli --verbose --log-level debug \
  "https://youtu.be/dQw4w9WgXcQ" > subtitle.txt 2> trace.log
```

VERIFY: `trace.log` contains `event = "retry"` lines with attempt numbers and the chosen provider, plus HTTP status codes per request.

```bash
grep '"event":"retry"' trace.log
grep '"event":"http_response"' trace.log | tail
```



## How To wire into a CI/CD pipeline

PROBLEM: A CI job needs to download subtitles for a fixed list of videos and fail the build if any video is missing a transcript.

SOLUTION: Combine `--json`, `--no-fallback` for determinism, and a shell loop that checks the exit code per URL.

```bash
#!/usr/bin/env bash
set -euo pipefail

while IFS= read -r url; do
  if ! youtube-legend-cli --json --no-fallback --provider youtube-direct "$url" \
       > "out/$(echo "$url" | sed 's|.*/||;s|?.*||').json" 2> "logs/$(date +%s).log"; then
    echo "CI failure on $url" >&2
    exit 1
  fi
done < urls.txt
```

VERIFY: The job exits with code `0` when every URL produced a JSON envelope, exits with the CLI's code (`64`/`65`/`66`/`69`/`70`) when one URL failed, and the `out/` directory contains one JSON file per video.


## How To force the YouTube direct provider

PROBLEM: Third-party providers do not index the video, but it
has public captions on YouTube.

SOLUTION: Pass `--provider youtube-direct` to pin the native
provider and skip the fallback chain. The CLI then talks to
YouTube's public endpoint and emits a clean SRT.

```bash
youtube-legend-cli --provider youtube-direct \
  --language pt-BR \
  "https://youtu.be/<id>" > subtitle.srt
```

VERIFY: The SRT has YouTube-canonical timing cues, no
provider watermark, and the envelope's `provider` field reads
`youtube-direct`.

```bash
head -n 3 subtitle.srt
youtube-legend-cli --json "https://youtu.be/<id>" | jq -r .provider
```


## How To diagnose a player.js failure

PROBLEM: The video is signature-protected and the decipher
step fails. You need a structured diagnostic before retrying.

SOLUTION: Use the `youtube-direct-probe` companion binary.
It inspects the cached `base.js`, runs the decipher on a
synthetic signature, and prints a JSON report.

```bash
youtube-direct-probe <video-id>
```

VERIFY: The probe prints one JSON object per line with
`signature_status`, `player_js_version`, `cache_hit`, and an
optional `decipher_error` field on failure.

```bash
youtube-direct-probe <video-id> | jq -r '.signature_status'
```

