[English](INTEGRATIONS.md) | [Português Brasileiro](INTEGRATIONS.pt-BR.md)
# Integrations — youtube-legend-cli

> Pipe a YouTube URL into a clean subtitle file. No daemon, no prompts, no telemetry.

[English](INTEGRATIONS.md) | [Português Brasileiro](INTEGRATIONS.pt-BR.md)

This page is the integration surface for AI agents, orchestrators, and CI
pipelines. It documents which flags an external caller can rely on, how
the environment overrides the CLI, and which flags shipped in which
release. The user-facing walkthrough lives in
[`docs/HOW_TO_USE.md`](docs/HOW_TO_USE.md).

## Compatible Agents and Orchestrators

The CLI is a single static binary with a native Unix `stdin`/`stdout`
contract. That means any agent that can spawn a subprocess and read
two streams can drive it. The following integrations are the ones
called out in the README and CI matrix.

### Claude Code

Claude Code is the maintainer's primary development surface and
treats `youtube-legend-cli` as a subprocess tool. The `--json` flag
emits a stable envelope on `stdout` while logs and progress stay on
`stderr`, so the agent can pipe the body straight into the next
tool call.

```bash
youtube-legend-cli --json "https://youtu.be/NvZ4VZ5hooY" \
  | jq '.body'
```

The companion `snapshot` binary probes both providers in isolation
and is the harness Claude Code uses when verifying that the v0.2.x
provider chain still returns clean subtitle bodies.

### GitHub Actions

The CI matrix in `ci.yml` already drives the CLI from a workflow
file. Pin the action to a specific release tag and surface `--json`
output as a build artefact when the job needs to assert on the
response shape.

```yaml
- name: Fetch subtitles
  run: |
    cargo install youtube-legend-cli --locked
    youtube-legend-cli --json "${{ inputs.url }}" > subtitle.json
- name: Verify body length
  run: |
    body_len=$(jq '.body | length' subtitle.json)
    test "$body_len" -gt 0
```

### Aider

Aider can call the CLI through its shell tool. Use `--batch` with
one URL per line so a single subprocess invocation covers every URL
Aider collected from the conversation.

```bash
printf '%s\n' \
  "https://youtu.be/NvZ4VZ5hooY" \
  "https://youtu.be/dQw4w9WgXcQ" \
  | youtube-legend-cli --batch
```

### Continue

Continue runs in VS Code and inherits the same shell semantics as
any Unix subprocess. The `~/.cache/youtube-legend-cli/` cache means
that re-running the same query inside an open editor session does
not re-hit the upstream provider until the TTL elapses.

### Cline

Cline is a VS Code extension that exposes a shell action. The
recommended pattern is to set `--quiet` so the agent transcript
stays clean while the subtitle body still arrives on `stdout`.

```bash
youtube-legend-cli --quiet --format srt \
  "https://youtu.be/NvZ4VZ5hooY" > subtitle.srt
```

### Codex

Codex is the OpenAI CLI companion. Like Aider, it can call the
binary through its shell tool. The `--config` flag accepts a TOML
file so a Codex session can swap providers or cache TTL without
re-typing long flag sets.

```bash
youtube-legend-cli --config ./yt-legend.toml \
  "https://youtu.be/NvZ4VZ5hooY"
```

## Flag Aliases

The clap-derived `Cli` struct exposes 17 flags. Three of them have
companion environment overrides that an orchestrator can set without
modifying the subprocess command line.

| Flag | Env override | Notes |
|------|--------------|-------|
| `--json` | — | CLI flag only. Emits a structured envelope on `stdout`. |
| `--log-level` | `YT_LOG_LEVEL` | `tracing-subscriber` reads `EnvFilter` first, so the env var wins when set. |
| `--log-format` | `YT_LOG_FORMAT` | Accepts `text` or `json`. The env var is the canonical way to enable JSON logs in CI. |

The `tracing-subscriber` initialiser in `src/logging.rs` is the
authoritative source for env precedence. When an integration needs
deterministic log shape, set `YT_LOG_FORMAT=json` rather than
relying on the flag.

## New Flags by Version

The flag surface is stable. Each release notes its additions in
`CHANGELOG.md`; the table below summarises the changes that are
relevant to integration authors.

| Version | New flags | Notes |
|---------|-----------|-------|
| v0.2.6 | `--config`, `--log-level`, `--log-format`, `--color`, `--no-progress`, `--dry-run`, `--yes` | The seven global flags were promoted in the Agent Teams playbook release. Every previous release already shipped `--lang`, `--format`, `--timeout`, `--verbose`, `--quiet`, `--json`, `--batch`, `--user-agent`, `--cache-ttl`, `--no-cache`. |
| v0.2.7 | — | No new flags. The release fixed the crates.io category slug. |
| v0.2.8 | — | No new flags. The release exposed `secret_endpoints.rs` to the source tree. |
| v0.2.9 | — | No new flags. The release lowered the MSRV to 1.88.0 in `rust-version`. |
| v0.3.0 | `--provider`, `--asr`, `--no-fallback` | Lands the YouTube-direct provider. `--provider` accepts `auto` (default), `youtube-direct`, `provider_a`, `provider_b`, or `provider_headless`. `--asr` is rejected with `EX_USAGE` when combined with `provider_a` or `provider_b`. |

## Summary Table

The table below is the single page an agent should bookmark. Every
flag that influences an integration is here, together with its
default, its environment companion where one exists, and a one-line
description of the consumer-visible effect.

| Flag | Env | Default | Effect on integration |
|------|-----|---------|-----------------------|
| `--config` | — | none | Path to a TOML config file. |
| `--log-level` | `YT_LOG_LEVEL` | `warn` | Tracing verbosity. Env wins. |
| `--log-format` | `YT_LOG_FORMAT` | `text` | `text` or `json` log shape. Env wins. |
| `--color` | — | `auto` | TTY-aware colour. Set `never` in CI. |
| `--no-progress` | — | `false` | Suppress progress bars on `stderr`. |
| `--dry-run` | — | `false` | Skip network I/O; serve from cache only. |
| `--yes` | — | `false` | Assume yes for any confirmation prompt. |
| `--lang` | — | `en` | BCP 47 tag, e.g. `pt-BR`. |
| `--format` | — | `txt` | `txt` (plain) or `srt` (preserved). |
| `--timeout` | — | `30` | HTTP timeout in seconds. |
| `--verbose` | — | `false` | Emit tracing events to `stderr`. |
| `--quiet` | — | `false` | Suppress all non-error `stderr`. |
| `--json` | — | `false` | Emit JSON envelope on `stdout`. |
| `--batch` | — | `false` | Read multiple URLs from `stdin`. |
| `--user-agent` | — | crate name | Override the default User-Agent. |
| `--cache-ttl` | — | `24` | Cache TTL in hours. |
| `--no-cache` | — | `false` | Skip cache reads. |
| `--provider` | — | `auto` | v0.3.0+. `auto`, `youtube-direct`, `provider_a`, `provider_b`, `provider_headless`. |
| `--asr` | — | `false` | v0.3.0+. Prefer the auto-generated caption track. |
| `--no-fallback` | — | `false` | v0.3.0+. Restrict the chain to the chosen provider. |

The exit-code table follows the BSD `sysexits.h` convention so any
POSIX-aware orchestrator can branch on category without parsing the
human-readable message. The full table is in
[`README.md`](README.md#exit-codes); the short version is `0` for
success, `64` for invalid usage, `65` for invalid URL, `66` for
no subtitle, `69` for upstream unavailable, `70` for internal
error, `78` for config error, and `130` for cooperative shutdown on
`SIGINT`/`SIGTERM`.
