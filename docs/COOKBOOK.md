# COOKBOOK

> Practical recipes for driving the subtitle CLI from a shell, a CI runner, or a Python pipeline.

Languages: [English](docs/COOKBOOK.md) | [Português Brasileiro](docs/COOKBOOK.pt-BR.md)

## Latency Note

The cache layer lives at `~/.cache/youtube-legend-cli/`. On a warm cache the body is served from disk in roughly one millisecond. On a cold cache the latency is dominated by the headless Chromium startup and page navigation: `provider-noteey` averages 15 s to 30 s end-to-end on a cold browser launch, and 5 s to 10 s on subsequent calls when the browser profile is warm.

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

PROBLEM: A pipeline needs the structured fields (`video_id`, `language`, `byte_size`, `content`) without writing a regex.

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
if "error" in envelope and envelope["error"]:
    raise SystemExit(f"provider error: {envelope['message']}")
print(envelope["content"])
```

VERIFY: The script prints the transcript to `stdout` and exits with code `0`. If the upstream is unavailable, `envelope["error"]` is a structured object and the script exits non-zero.


## How To switch providers for CI

PROBLEM: CI runs need a deterministic provider to avoid flakes when one upstream is degraded.

SOLUTION: Since v0.3.2 the CLI uses a single provider (`provider-noteey`). Pass `--provider provider-noteey` explicitly if you want to document the choice in CI. The CLI exits with `69` if the provider fails.

```bash
youtube-legend-cli --provider provider-noteey \
  "https://youtu.be/dQw4w9WgXcQ" > subtitle.txt
```

VERIFY: The exit code is `0` on success, `69` on upstream unavailability.


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

SOLUTION: Combine `--json` and `--provider provider-noteey` for determinism, and a shell loop that checks the exit code per URL.

```bash
#!/usr/bin/env bash
set -euo pipefail

while IFS= read -r url; do
  if ! youtube-legend-cli --json --provider provider-noteey "$url" \
       > "out/$(echo "$url" | sed 's|.*/||;s|?.*||').json" 2> "logs/$(date +%s).log"; then
    echo "CI failure on $url" >&2
    exit 1
  fi
done < urls.txt
```

VERIFY: The job exits with code `0` when every URL produced a JSON envelope, exits with the CLI's code (`64`/`65`/`66`/`69`/`70`) when one URL failed, and the `out/` directory contains one JSON file per video.


## How To use provider-noteey explicitly

PROBLEM: You want to document the provider choice in your pipeline script.

SOLUTION: Pass `--provider provider-noteey` to pin the provider explicitly. Since v0.3.2 this is the only provider, so `auto` resolves to the same path.

```bash
youtube-legend-cli --provider provider-noteey \
  --lang pt \
  "https://youtu.be/<id>" > subtitle.txt
```

VERIFY: The output is a clean transcript and the JSON envelope's `provider` field reads `provider-noteey`.

```bash
youtube-legend-cli --json --provider provider-noteey "https://youtu.be/<id>" \
  | jq -r .provider
```

