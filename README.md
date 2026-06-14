# youtube-legend-cli

[![docs.rs](https://docs.rs/youtube-legend-cli/badge.svg)](https://docs.rs/youtube-legend-cli)
[![Crates.io](https://img.shields.io/crates/v/youtube-legend-cli.svg)](https://crates.io/crates/youtube-legend-cli)
[![v0.2.6](https://img.shields.io/badge/release-v0.2.6-blue.svg)](CHANGELOG.md)
[![License: MIT OR Apache-2.0](https://img.shields.io/crates/l/youtube-legend-cli.svg)](LICENSE)
[![MSRV 1.96.0](https://img.shields.io/badge/MSRV-1.96.0-blue.svg)](rust-toolchain.toml)
[![CI](https://github.com/danilo/youtube-legend-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/danilo/youtube-legend-cli/actions/workflows/ci.yml)
[![Downloads](https://img.shields.io/crates/d/youtube-legend-cli.svg)](https://crates.io/crates/youtube-legend-cli)
[![Rust 1.96+](https://img.shields.io/badge/rust-1.96%2B-orange.svg)](https://www.rust-lang.org)

Non-interactive Rust CLI that downloads YouTube subtitles through
third-party providers, using a native Unix `stdin` / `stdout`
interface. Single static binary, no daemon, no telemetry.

## Overview

`youtube-legend-cli` is a single static Rust binary that turns any
YouTube URL into a clean subtitle file. It is non-interactive, has no
daemon, and never phones home. The interface is pure Unix: one URL on
`stdin` (or as a positional argument), the subtitle body on `stdout`,
and all logs and progress on `stderr`.

## Features

- Two-provider extraction pipeline (`provider_a`, `provider_b`) with
  automatic fallback.
- Local file cache keyed on `(video_id, language, format)` with
  configurable TTL (default 24h).
- Batch mode reading one URL per line from `stdin`.
- Structured JSON envelope on `stdout` via `--json`.
- Exponential backoff (1s, 2s, 4s) with per-provider circuit breaker.
- Unicode NFC normalisation and SRT-to-text conversion.
- AES-256-CBC plus PBKDF2 token signing for the `provider_b`
  compatibility path.
- 50 MiB in-memory safety cap on decoded subtitle size.
- Graceful `SIGINT` and `SIGTERM` handling, exits with code 130.
- Zero telemetry: no analytics, no network call home.

## Quickstart

```bash
# Install from crates.io
cargo install youtube-legend-cli

# Or build from source
cargo build --release

# Download subtitles for one video
youtube-legend-cli "https://youtu.be/NvZ4VZ5hooY" > subtitle.txt

# Structured JSON output
youtube-legend-cli --json "https://youtu.be/NvZ4VZ5hooY"

# Batch mode from stdin
cat urls.txt | youtube-legend-cli --batch > subtitles.txt

# Specific language
youtube-legend-cli --lang pt "https://youtu.be/NvZ4VZ5hooY"
```

## Examples

```bash
# One URL, plain text output
youtube-legend-cli "https://youtu.be/dQw4w9WgXcQ" > subtitle.txt

# Preserve SRT timestamps
youtube-legend-cli --format srt "https://youtu.be/dQw4w9WgXcQ" > subtitle.srt

# Brazilian Portuguese
youtube-legend-cli --lang pt "https://youtu.be/dQw4w9WgXcQ"

# Batch from a file
youtube-legend-cli --batch < urls.txt > subtitles.txt

# JSON envelope on stdout, logs on stderr
youtube-legend-cli --json --verbose "https://youtu.be/dQw4w9WgXcQ"
```

## Targets

Pre-built binaries are produced and tested for:

- `x86_64-unknown-linux-gnu` (glibc dynamic)
- `x86_64-unknown-linux-musl` (fully static)
- `aarch64-unknown-linux-musl` (ARM64 static)
- `x86_64-pc-windows-msvc` (Windows 64-bit)
- `x86_64-apple-darwin` (cross-compile via `osxcross`, `continue-on-error: true` in CI)
- `aarch64-apple-darwin` (cross-compile via `osxcross`, `continue-on-error: true` in CI)

The `aarch64-apple-darwin` target is in the CI matrix but is best
built on a host that ships `osxcross`; the source tree itself is
portable to any Tier-1 Rust target.

## Companion binaries

The crate ships two binaries:

- `youtube-legend-cli` — the subtitle fetcher (default).
- `snapshot` — probes both providers and writes redacted HTML
  snapshots under `tests/fixtures/snapshots/<date>/` for drift
  detection. The gitignored `src/secret_endpoints.rs` is consumed
  via `#[path = "..."]` so the upstream hostnames never enter the
  published rustdoc. Run with `cargo run --bin snapshot`.

## MSRV

The Minimum Supported Rust Version is **1.96.0**, pinned in
`rust-toolchain.toml`. The MSRV job in CI builds and tests the crate
on this version on every push.

## Stream contracts

- `stdout` is reserved exclusively for the subtitle body (or the
  `--json` envelope).
- `stderr` is reserved exclusively for logs, progress, and human
  error messages.
- `stdin` accepts a single URL, a batch of one URL per line, or
  `--batch` flag input.

## Flags

| Flag             | Description                                  | Default     |
|------------------|----------------------------------------------|-------------|
| `--lang`         | `en`, `pt`, `es`, `fr`, `de`, `it`, or BCP 47 forms such as `pt-BR` / `pt_BR.UTF-8` | `en`        |
| `--format`       | `txt` (plain) or `srt` (preserved)           | `txt`       |
| `--timeout`      | HTTP timeout in seconds                      | `30`        |
| `--verbose`      | Emit tracing events to stderr                | `false`     |
| `--quiet`        | Suppress all non-error stderr                | `false`     |
| `--json`         | Emit JSON envelope to stdout                 | `false`     |
| `--batch`        | Read multiple URLs from stdin                | `false`     |
| `--user-agent`   | Override the default User-Agent              | crate name  |
| `--cache-ttl`    | Cache TTL in hours                           | `24`        |
| `--no-cache`     | Skip cache reads                             | `false`     |
| `--config`       | Path to a TOML config file                   | none        |
| `--log-level`    | `error`, `warn`, `info`, `debug`, `trace`    | `warn`      |
| `--log-format`   | `text` or `json`                             | `text`      |
| `--color`        | `auto`, `always`, `never`                    | `auto`      |
| `--no-progress`  | Suppress progress bars on stderr             | `false`     |
| `--dry-run`      | Skip network I/O; serve reads from cache only | `false`    |
| `--yes`          | Assume yes for any confirmation prompt       | `false`     |

## Exit codes

The CLI follows the BSD `sysexits.h` convention so downstream POSIX
tooling can branch on category. See [`src/error.rs`](src/error.rs) for
the canonical mapping.

| Code | Meaning                                            |
|------|----------------------------------------------------|
| `0`  | Success                                            |
| `64` | Invalid usage or input (`EX_USAGE`)                |
| `65` | Invalid URL (`EX_DATAERR`)                         |
| `66` | No subtitle for the video (`EX_NOINPUT`)           |
| `69` | All providers unavailable, or rate limited, or `robots.txt` `Disallow` (`EX_UNAVAILABLE`) |
| `70` | Internal / I/O / HTTP / timeout / crypto error (`EX_SOFTWARE`) |
| `78` | Configuration error in `--config` TOML (`EX_CONFIG`) |
| `130`| Received `SIGINT` / `SIGTERM` (first signal cooperative, second signal forces exit) |

On HTTP 429 the CLI honours the `Retry-After` header in both delta-seconds
and RFC 2822 HTTP-date form (60 s fallback when absent, capped at 300 s)
before retrying.

## Installation

```bash
# From crates.io
cargo install youtube-legend-cli

# From the local checkout
cargo install --path .

# Verify
youtube-legend-cli --version
```

Requires Rust 1.96.0 or newer. See `rust-toolchain.toml`.

## Performance baseline

Three micro-benchmarks live in `benches/cache_bench.rs`:

- `cache_key_compose` — composes cache filename from (video_id, lang, format)
- `url_length_check` — validates URL against 2048-byte cap
- `locale_parse_primary_subtag` — normalises BCP 47 to ISO 639-1


Run with `cargo bench --bench cache_bench`. Baseline on the maintainer's
x86_64-unknown-linux-gnu host (2026-06-14, release profile, 1000 samples):
- `cache_key_compose`: ~34 ns/iter
- `url_length_check`: 0 ns/iter (sub-ns, rounded down)
- `locale_parse_primary_subtag`: ~6 ns/iter

## Documentation

- [docs.rs/youtube-legend-cli](https://docs.rs/youtube-legend-cli) —
  API reference for every public item.
- [CHANGELOG.md](CHANGELOG.md) — release history.
- [CONTRIBUTING.md](CONTRIBUTING.md) — development workflow.
- [SECURITY.md](SECURITY.md) — vulnerability disclosure.
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) — community standards.
- [llms.txt](llms.txt) — LLM-friendly entry point.
- [llms-full.txt](llms-full.txt) — LLM-friendly full reference.
- [docs/agent-teams-workflow.md](docs/agent-teams-workflow.md) —
  the Agent Teams playbook used to deliver v0.2.6.
- [docs/decisions/0009-cargo-toml-ownership-in-parallel-tasks.md](docs/decisions/0009-cargo-toml-ownership-in-parallel-tasks.md) —
  ADR-0009 (Cargo.toml serialisation under Agent Teams).
- `docs_prd/prd_youtube-legend-cli.md` — full PRD (Constitution: PRINC-001 a PRINC-015 embedded at §13).
- `docs_prd/spec_tecnica.md` — module contracts.
- `docs_prd/plano_implementacao.md` — development phases.

## Security

See [`SECURITY.md`](SECURITY.md) for the supported versions table,
the threat model, and the private vulnerability disclosure channel.

## Code of Conduct

This project follows the [Contributor Covenant 2.1](CODE_OF_CONDUCT.md).

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the development
workflow, MSRV expectations, style rules, and the
`no Co-authored-by` policy.

## License

Dual-licensed under either of [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE), at your option.
