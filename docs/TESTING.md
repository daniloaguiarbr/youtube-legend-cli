[English](TESTING.md) | [Português Brasileiro](TESTING.pt-BR.md)
# Testing Guide — youtube-legend-cli

> A categorized test suite that mirrors the provider pipeline.

## Why Categorized Tests

`youtube-legend-cli` ships with one binary, one library crate,
three examples, and one benchmark. The tests are split into four
categories that match the build and runtime surface:

- UNIT TESTS under `#[cfg(test)]` modules inside `src/`.
  Fast, deterministic, no network. Exercised by `cargo test --lib`.
- DOC TESTS in the rustdoc comments. Exercised by
  `cargo test --doc`. Code blocks in `///` and `//!` are
  compiled and run.
- INTEGRATION TESTS under `tests/integration/`. Cross-crate
  surface, may need network or wiremock. Exercised by
  `cargo test --test <name>`.
- BENCHMARKS under `benches/`. Criterion-based micro-benchmarks
  for the hot path. Exercised by `cargo bench --bench cache_bench`.

The split exists because the failure modes are different. A unit
test that hits the network is a flake waiting to happen. An
integration test that ships in the public crate and runs on every
`cargo test` invocation is a CI bottleneck. Categorizing lets us
make the trade-off explicit.

## Test Categories

Seven integration tests live under `tests/integration/`:

| Test | Purpose | Network? | Run by default? |
|---|---|---|---|
| `corpus` | Smoke test on a corpus of real YouTube URLs | Yes | No — `--include-ignored` |
| `rss` | Enforces NFR-002 RSS budget of 100 MiB | No | Yes |
| `offline_cache` | Cache hit round-trip with no network (NFR-005) | No | Yes |
| `provider_a_wiremock` | Provider A against `wiremock` mocks | No (mock) | Yes |
| `provider_b_wiremock` | Provider B against `wiremock` mocks | No (mock) | Yes |
| `signal_handler_stress` | `SIGINT` / `SIGTERM` under stress | No | No — `--include-ignored` |
| `cli_probing` | CLI flags and exit codes | No | Yes |

The two `--include-ignored` tests are gated in the CI:

- `corpus` — runs in the `test` job with `continue-on-error: true`
  because real YouTube URLs can rate-limit or change shape.
- `signal_handler_stress` — runs only on Linux runners because
  signal delivery semantics differ on macOS and Windows.

## How to Run

### Unit tests

```bash
cargo test --lib
```

Runs every `#[cfg(test)]` module. Should finish in under 30 seconds
on a warm cache.

### Doc tests

```bash
cargo test --doc
```

Runs every code block in the rustdoc. Should finish in under 60
seconds on a warm cache.

### Integration tests, one by one

```bash
cargo test --test corpus
cargo test --test rss
cargo test --test offline_cache
cargo test --test provider_a_wiremock
cargo test --test provider_b_wiremock
cargo test --test signal_handler_stress
cargo test --test cli_probing
```

### Integration tests, all at once

```bash
cargo test --tests
```

Skips doctests and lib tests. Useful when iterating on a single
integration suite.

### Integration tests, including gated ones

```bash
cargo test --test corpus -- --include-ignored
cargo test --test signal_handler_stress -- --include-ignored
```

The `--include-ignored` flag is the standard `cargo test` way to
run `#[ignore]`-gated cases. The CI does this for the `corpus`
test in the `test` job.

### Benchmarks

```bash
cargo bench --bench cache_bench
```

Three Criterion micro-benchmarks: cache key composer, URL length
check, BCP 47 locale parser. CI verifies the target compiles via
`cargo bench --no-run`; the full bench runs only on demand.

## CI Profiles

The `.github/workflows/ci.yml` file runs twelve jobs. Each job
maps to a specific quality gate:

| Job | Profile | Hardware | What it does |
|---|---|---|---|
| `test` | stable + beta matrix | `ubuntu-latest` | fmt, clippy, build, unit, doc, integration gated corpus, RSS gate, offline cache, wiremock, example smoke, binary size, --help/--version |
| `cross-compile` | 6 targets | `ubuntu-latest` | `cargo build --release --target <triple>` |
| `publish-dry-run` | stable | `ubuntu-latest` | `cargo package --list` + `cargo publish --dry-run` |
| `msrv` | rustc 1.96.0 | `ubuntu-latest` | `cargo build --locked` + `cargo test --lib --locked` |
| `deny` | stable | `ubuntu-latest` | `cargo deny check` (licenses, bans, advisories) |
| `audit` | stable | `ubuntu-latest` | `cargo audit` for known vulnerabilities |
| `public-api` | stable | `ubuntu-latest` | `cargo public-api` baseline + sigilo gate + PR diff |
| `semver-checks` | stable | `ubuntu-latest` | `cargo semver-checks --all-features` |
| `cargo-install` | stable | `ubuntu-latest` | `cargo install --path` + --version + --help |
| `matrix-os` | stable on 3 OSes | `ubuntu-latest`, `macos-latest`, `windows-latest` | clippy + build per OS |
| `nightly` | nightly | `ubuntu-latest` | clippy + doc build on the unstable toolchain |
| `docs-link-check` | stable | `ubuntu-latest` | `cargo doc` + `lychee --offline target/doc/` |

The `matrix-os` job is the only one that exercises real Apple
silicon and real Windows; the rest of the matrix is Linux-based.

## Environment Variables

The test suite respects the standard Rust test environment plus
a few project-specific knobs:

- `TEST_INTEGRATION` — when set to `1`, runs the `corpus` test
  even on local development. By default the corpus test is
  `#[ignore]`-gated.
- `RUST_LOG` — `tracing` env filter. Useful values are
  `info`, `youtube_legend_cli=debug`, or `warn` for quiet output.
  The test suite emits structured tracing events to stderr.
- `RUSTFLAGS` — the CI sets `-D warnings`. Local development
  without this is fine; CI will reject warnings.
- `RUSTDOCFLAGS` — the CI sets `-D warnings`. Doc tests fail
  on broken intra-doc links.
- `CARGO_TERM_COLOR` — `always` in CI; `auto` locally.
- `HTTP_PROXY` / `HTTPS_PROXY` — honoured by the `reqwest` HTTP
  client. Useful for capturing the upstream traffic during
  local debugging of the wiremock tests.
- `WIREMOCK_PRINT_RESPONSES` — set to `1` to dump the mock
  server's responses to stderr. Handy when a `provider_a_wiremock`
  assertion is failing and you need to see the actual payload.
- `YT_LEGEND_CACHE_DIR` — overrides the default cache directory.
  The integration tests set this to a `tempdir` so they never
  pollute the real `~/.cache/youtube-legend-cli/`.
- `YT_LEGEND_NO_NETWORK` — set to `1` to fail any test that
  attempts an outbound connection. The offline cache test
  asserts on this.

## Troubleshooting

### Flaky `corpus` test

The `corpus` test hits real YouTube URLs. Rate limits, transient
network failures, and upstream HTML drift can all cause
spurious failures. The CI runs the test with
`continue-on-error: true` and prints the failure for triage.

If you see a failure locally, set `TEST_INTEGRATION=1` to
re-enable the test, then run with
`RUST_LOG=youtube_legend_cli=trace` to see the full HTTP
exchange. The expected behaviour is one of:

- All URLs return a non-empty subtitle body.
- A specific URL returns `EX_NOINPUT` (`66`) because YouTube
  removed captions. Add it to the skip list in `corpus.rs`.

### `signal_handler_stress` only runs on Linux

Signal delivery on macOS and Windows differs enough that the
test is `#[cfg(target_os = "linux")]`-gated. The CI runs it on
the `ubuntu-latest` runner in the `test` job. Do not be alarmed
when `cargo test --test signal_handler_stress` is a no-op on
your Mac.

### `cargo test` on macOS is slow

The `reqwest` `rustls` feature uses the platform-native crypto
backend. On macOS that is `Secure Transport`, which is slow for
the first connection. Subsequent runs hit the connection pool.
This is not a `youtube-legend-cli` regression.

### `cargo test --doc` reports broken intra-doc links

The CI sets `RUSTDOCFLAGS="-D warnings"` so any broken link is a
build failure. To find the offender locally:

```bash
RUSTDOCFLAGS="-D warnings" cargo test --doc
```

The error message points at the offending `///` block. Fix the
path or use a relative URL.

### `cargo bench` aborts on missing Criterion

The benchmark target requires `criterion = "0.5"` in
`[dev-dependencies]`. If you see a compile error mentioning
`criterion`, run `cargo build --benches` to force the dev
dependency download.

## ProviderYouTubeDirect Tests (v0.3.0)

Categories added in v0.3.0:

- `unit_youtube`: tests `player_response`, `decipher`, `ncode`,
  and `caption_track` in isolation against fixtures.
- `integration_youtube`: drives the provider end-to-end against
  frozen HTML snapshots.
- `integration_srv3`: exercises the Srv3/Json3 parser with
  fixtures under `tests/fixtures/timedtext/*.srv3`.

### How To Run

```bash
cargo test --features youtube-direct
```

### Coverage Target

Expectation: above 80 percent line coverage on the new modules
(`src/provider/youtube/`, `src/parse/srv3.rs`).

## See Also

- [README](../README.md) — install and run.
- [docs/ARCHITECTURE.md](ARCHITECTURE.md) — module map and provider
  pipeline.
- [docs/CROSS_PLATFORM.md](CROSS_PLATFORM.md) — six cross-compile
  targets.
- [docs/MIGRATION.md](MIGRATION.md) — v0.2.9 to v0.3.0 changes.
