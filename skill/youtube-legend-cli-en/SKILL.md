---
name: youtube-legend-cli
description: Triggers when the user asks to download YouTube subtitles, captions, transcripts, or text tracks from a YouTube URL (watch, shorts, embed, youtu.be). Also triggers on mentions of yt-dlp replacement, youtube-legend-cli, downsub alternative, noteey, save subs, batch subtitle download, or headless browser subtitle extraction. This skill MUST be used to invoke the youtube-legend-cli Rust CLI for non-interactive, scriptable, JSON-enveloped subtitle retrieval via the provider-noteey headless-Chromium pipeline. Covers all 18 CLI flags with ready-to-use formulas, JSON envelope fields (content, language_detected, byte_size, video_id, duration_ms), BSD sysexits exit codes, NDJSON batch output, TTL cache, retry handling, BrowserFetcher auto-download, CHROME env override, environment variables, TOML config, and offline mode.
---


# youtube-legend-cli


## Identity and Architecture
- youtube-legend-cli is a non-interactive Rust CLI that downloads YouTube subtitles
- The CLI uses EXACTLY ONE provider called `provider-noteey` (noteey.com via headless Chromium)
- `--provider auto` resolves to `provider-noteey` without fallback
- noteey.com returns ONE transcript per page in the original video language without language selection
- The parser strips timestamps, `[Music]` annotation markers, and `>>` speaker change markers
- The parser normalises all output to Unicode NFC
- stdout carries ONLY the subtitle text or the `--json` envelope
- stderr carries ONLY logs, progress bars, and diagnostics
- ALWAYS discard stderr before piping stdout into `jaq`
- `--format srt` is UNAVAILABLE with provider-noteey and returns exit 64
- Install with `cargo install youtube-legend-cli` — MSRV is Rust 1.88


## CLI Flags
- PASS `--json` for every programmatic consumer
- `--lang` accepts ISO 639-1 codes, full BCP 47 locales (`pt-BR`), or POSIX locales (`pt_BR.UTF-8`) and normalises to the primary subtag
- `--format` accepts `txt` (default) — `srt` is UNAVAILABLE and returns exit 64
- `--provider` accepts ONLY `auto` (default) or `provider-noteey`
- `--timeout` accepts positive integer seconds for the HTTP bound (default 30)
- `--config <PATH>` loads a flat TOML configuration file
- `--cache-ttl` accepts positive integer hours to override the TTL (default 24)
- `--no-cache` forces a fresh read ignoring the local cache
- `--no-progress` suppresses progress bars on stderr
- `--dry-run` skips network I/O and serves reads from cache only
- `--yes` assumes yes for non-interactive prompts
- `--batch` reads URLs from stdin one per line — emits NDJSON when combined with `--json`
- `--user-agent` overrides the HTTP User-Agent header
- `--verbose` activates INFO-level logging on stderr
- `--quiet` suppresses all non-error log output on stderr
- `--verbose` and `--quiet` are mutually exclusive
- `--log-level` accepts `error`, `warn`, `info`, `debug`, `trace`
- `--log-format` accepts `text` (default) or `json` for structured log output
- `--color` accepts `auto`, `always`, `never` — use `never` in CI pipelines
- NEVER pass removed providers: `youtube-direct`, `provider-a`, `provider-b`, `provider-headless`
- NEVER pass removed flags: `--asr`, `--no-fallback`, `--headless`
- NEVER combine `--batch` with a positional URL — exit 64
- NEVER combine `--quiet` with `--verbose` — exit 64
- NEVER pass `--timeout 0` or `--cache-ttl 0` — exit 64
- NEVER hardcode hostnames in scripts


## JSON Envelope and NDJSON
- VALIDATE the `error` field BEFORE trusting any other field
- Success fields: `provider` (provider-noteey or cache), `video_id`, `language` (requested locale), `language_detected` (ALWAYS false), `format`, `content` (clean NFC text), `byte_size`, `duration_ms` (wall-clock ms), `source_url`
- `language_detected` is ALWAYS false because noteey.com has NO language selector
- `byte_size` reflects EXACT byte length of `content` after parsing and NFC normalisation
- `duration_ms` reflects wall-clock milliseconds from request start to response completion
- `source_url` contains the original YouTube URL as submitted to the CLI
- `video_id` contains the 11-character YouTube video identifier extracted from the URL
- Error fields: `error` (always true), `code` (BSD sysexits integer), `message` (human-readable string)
- ALL errors emit structured JSON on stdout when `--json` is active INCLUDING pre-fetch validation errors
- `--batch --json` emits NDJSON (one JSON object per line, newline-terminated)
- Each NDJSON object is self-contained and parseable independently
- MUST parse NDJSON line-by-line — `jaq -c 'select(.error == null)'` filters per-line
- READ `retry_after_seconds` when present in error envelopes
- NEVER read `.body` — the field is `.content`
- NEVER parse stdout line-by-line as raw text when `--json` is active
- NEVER assume `content` is always non-empty
- NEVER assume batch output is a JSON array — it is NDJSON


## Exit Codes
- `0` — success
- `2` — clap parser rejection on invalid flag value
- `64` EX_USAGE — invalid flag combinations, `--format srt` with provider-noteey
- `65` EX_DATAERR — malformed or unrecognised YouTube URL
- `66` EX_NOINPUT — video has no matching subtitle
- `69` EX_UNAVAILABLE — provider unavailable, rate-limited, Chromium missing, or captcha
- `70` EX_SOFTWARE — internal failure, timeout, HTTP, I/O, or parse errors
- `78` EX_CONFIG — malformed or unreadable config file
- `130` — SIGINT or SIGTERM user interrupt
- NEVER mask the exit code with `|| true`
- NEVER treat `69` as permanent — Chromium absence and captcha are recoverable


## Provider and Chromium
- The EXCLUSIVE provider is `provider-noteey` driving noteey.com through headless Chromium
- The provider navigates to noteey.com, fills the URL input, clicks "Get Subtitle", and polls the transcript pane for up to 30 seconds
- noteey.com returns ONE transcript in the ORIGINAL video language — NO language selector exists
- Anti-fingerprint stealth patches are injected via CDP before navigation
- Chromium resolution order: (1) `$CHROME` env var, (2) BrowserFetcher auto-download r1585606 to `~/.cache/youtube-legend-cli/browser/`, (3) well-known system paths
- PREFER the BrowserFetcher revision over a system browser to avoid CDP protocol mismatches
- SET `$CHROME` to pin a compatible binary and skip the download
- Chrome profile lives at `~/.cache/youtube-legend-cli/chrome-profile/`
- NEVER expect `--format srt` to work — returns exit 64
- NEVER expect noteey to return subtitles in a specific language — it returns the original language only
- NEVER expect a captcha to resolve by retrying — returns exit 69


## Cache and Retry
- 24-hour default TTL on disk at `~/.cache/youtube-legend-cli/`
- Override the cache root with `$YT_LEGEND_CACHE_DIR`
- USE `--no-cache` for fresh fetches in audit pipelines
- USE `--cache-ttl` to override the TTL in positive integer hours
- USE `--dry-run` to serve cached results without network I/O
- Invalidate a single entry by removing its directory
- READ `retry_after_seconds` from error envelopes on HTTP 429
- Internal fallback is 60 seconds with a 300-second cap
- NEVER delete the entire cache directory in production scripts
- NEVER run client-side retry loops without backoff
- NEVER set `--timeout` below 5 seconds


## Environment Variables
- `CHROME` — pins the Chromium executable and skips BrowserFetcher download
- `YT_LEGEND_NO_NETWORK` — disables all network I/O, returns exit 69
- `YT_LOG_LEVEL` — wins over `--log-level`
- `YT_LOG_FORMAT` — wins over `--log-format`
- `YT_LEGEND_CACHE_DIR` — overrides the XDG cache directory
- `NO_COLOR` and `CLICOLOR_FORCE` are honoured when `--color` is `auto`
- USE the `YT_*` family for any configuration override
- NEVER set `RUST_LOG` directly when a `YT_*` override applies


## Config File (TOML)
- `--config <PATH>` loads a flat TOML table whose keys mirror long flag names without the leading `--`
- Precedence: CLI flag wins over config value wins over built-in default
- Supported keys: `url`, `lang`, `format`, `timeout`, `cache_ttl`, `user_agent`, `provider`
- Boolean keys: `verbose`, `quiet`, `json`, `batch`, `no_cache`, `dry_run`, `no_progress`, `yes`
- Optional keys: `log_level`, `log_format`, `color`
- A minimal config contains `lang = "en"`, `format = "txt"`, `cache_ttl = 24`
- NEVER add an unknown key — exit 78
- NEVER write malformed TOML — exit 78


## Error Handling
- BRANCH on exit code to determine the error category
- BrowserNotFound (exit 69) — install a browser or set `$CHROME`
- CaptchaChallenge (exit 69) — requires human interaction, CANNOT retry
- NoSubtitle (exit 66) — no transcript available for the video
- RateLimited (exit 69) — read `retry_after_seconds` and wait
- ALL errors emit structured JSON when `--json` is active
- EXTRACT error code from envelope: `jaq '.code'`
- EXTRACT error message from envelope: `jaq -r '.message'`
- NEVER panic on non-zero exit in pipeline logic
- NEVER stringify errors to match on substrings


## Ready-to-Use Formulas
- FLAG-BY-FLAG FORMULAS (all 18 flags)
- DOWNLOAD single video: `youtube-legend-cli "https://youtu.be/VIDEO" > subtitle.txt`
- EXTRACT content via JSON: `youtube-legend-cli --json "https://youtu.be/VIDEO" 2>/dev/null | jaq -r '.content'`
- SELECT language: `youtube-legend-cli --lang pt-BR "https://youtu.be/VIDEO" > legenda.txt`
- SELECT language with POSIX locale: `youtube-legend-cli --lang pt_BR.UTF-8 "https://youtu.be/VIDEO"`
- EXPLICIT txt format: `youtube-legend-cli --format txt "https://youtu.be/VIDEO" > clean.txt`
- SET custom timeout: `youtube-legend-cli --timeout 60 "https://youtu.be/VIDEO"`
- LOAD config file: `youtube-legend-cli --config ./yt-legend.toml "https://youtu.be/VIDEO"`
- OVERRIDE cache TTL: `youtube-legend-cli --cache-ttl 168 "https://youtu.be/VIDEO"`
- FORCE fresh fetch: `youtube-legend-cli --no-cache "https://youtu.be/VIDEO" > fresh.txt`
- SUPPRESS progress bars: `youtube-legend-cli --no-progress "https://youtu.be/VIDEO" > subtitle.txt 2>/dev/null`
- CACHE-ONLY dry run: `youtube-legend-cli --dry-run "https://youtu.be/VIDEO"`
- NON-INTERACTIVE batch: `youtube-legend-cli --yes --batch < urls.txt > out.txt`
- BATCH with NDJSON: `cat urls.txt | youtube-legend-cli --batch --json 2>/dev/null | jaq -r 'select(.error == null) | .content'`
- CUSTOM user-agent: `youtube-legend-cli --user-agent "MyBot/1.0" "https://youtu.be/VIDEO"`
- VERBOSE debug: `youtube-legend-cli --verbose --log-level debug "https://youtu.be/VIDEO" > sub.txt 2> trace.log`
- QUIET mode: `youtube-legend-cli --quiet "https://youtu.be/VIDEO" > subtitle.txt`
- JSON log format: `YT_LOG_FORMAT=json youtube-legend-cli --log-format json --json "https://youtu.be/VIDEO" 2> logs.jsonl`
- NO COLOR in CI: `youtube-legend-cli --color never --json "https://youtu.be/VIDEO"`
- PIN provider: `youtube-legend-cli --provider provider-noteey "https://youtu.be/VIDEO"`
- ENVELOPE FIELD EXTRACTION FORMULAS
- EXTRACT video_id: `youtube-legend-cli --json "URL" 2>/dev/null | jaq -r '.video_id'`
- CHECK language_detected: `youtube-legend-cli --json "URL" 2>/dev/null | jaq '.language_detected'`
- READ byte_size: `youtube-legend-cli --json "URL" 2>/dev/null | jaq '.byte_size'`
- READ duration_ms: `youtube-legend-cli --json "URL" 2>/dev/null | jaq '.duration_ms'`
- READ source_url: `youtube-legend-cli --json "URL" 2>/dev/null | jaq -r '.source_url'`
- READ provider: `youtube-legend-cli --json "URL" 2>/dev/null | jaq -r '.provider'`
- EXTRACT error code: `youtube-legend-cli --json "INVALID_URL" 2>/dev/null | jaq '.code'`
- EXTRACT error message: `youtube-legend-cli --json "INVALID_URL" 2>/dev/null | jaq -r '.message'`
- COMBINED PATTERN FORMULAS
- PARSE envelope safely: `out=$(youtube-legend-cli --json "$url" 2>/dev/null) && echo "$out" | jaq -e '.error == null' >/dev/null && echo "$out" | jaq -r '.content' || echo "$out" | jaq '{code: .code, message: .message}'`
- ROUTE by exit code: `youtube-legend-cli "$url" || case $? in 66) echo "no subtitles";; 69) echo "provider down";; *) echo "failed";; esac`
- CI fresh no-progress JSON: `youtube-legend-cli --json --no-cache --no-progress --provider provider-noteey "$url" > out.json 2> trace.log`
- BATCH quiet NDJSON: `cat urls.txt | youtube-legend-cli --batch --json --quiet --no-progress 2>/dev/null | jaq -c 'select(.error == null) | {id: .video_id, bytes: .byte_size}'`
- OFFLINE mode: `YT_LEGEND_NO_NETWORK=1 youtube-legend-cli "$url"` (returns exit 69)
- PIN Chromium: `CHROME=/usr/bin/chromium youtube-legend-cli "$url"`
- AUDIT fresh pipeline: `youtube-legend-cli --no-cache --json "$url" 2>/dev/null | jaq -r '.content' > fresh-audit.txt`
