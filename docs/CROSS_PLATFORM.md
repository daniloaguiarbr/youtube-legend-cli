[English](CROSS_PLATFORM.md) | [Português Brasileiro](CROSS_PLATFORM.pt-BR.md)
# Cross-Platform Guide — youtube-legend-cli

> Distribution that just works, on the box you actually run it on.

## The Pain You Already Know

You wrote a five-line shell pipeline that pulls a YouTube transcript
on your Linux laptop. Now somebody on macOS tries the same pipeline
and `cargo install` chokes on `aws-lc-sys`. The CI green check
did not save you. A teammate on Windows 11 gets a `LINK : fatal
error LNK1181: cannot open input file 'crypt32.lib'`. The agent
running on an Alpine container inside a distroless base has no
`glibc` and the static-binary build you shipped is segfaulting
before it ever talks to the network.

`youtube-legend-cli` was designed to make those days end. One
crate, six cross-compile targets, identical 17-flag CLI on every
platform, identical `~/.cache/youtube-legend-cli/` layout, and a
single source of truth (`Cargo.toml` `[package.metadata.docs.rs]`)
that docs.rs renders for the same six targets. This page is the
field guide.

## Support Matrix

| OS | Target triple | CI status | Runtime deps |
|---|---|---|---|
| Linux x86_64 (glibc) | `x86_64-unknown-linux-gnu` | Green on `ubuntu-latest` | `glibc >= 2.31` |
| Linux x86_64 (musl) | `x86_64-unknown-linux-musl` | Green on `ubuntu-latest` | None (static) |
| Linux ARM64 (musl) | `aarch64-unknown-linux-musl` | Green on `ubuntu-latest` | None (static) |
| Windows x86_64 | `x86_64-pc-windows-msvc` | Green via `cross-compile` job | `Microsoft Visual C++ 2015-2022 Redistributable` |
| macOS x86_64 (Intel) | `x86_64-apple-darwin` | `continue-on-error: true` (needs `osxcross`) | macOS 10.15+ |
| macOS ARM64 (Apple Silicon) | `aarch64-apple-darwin` | `continue-on-error: true` (needs `osxcross`) | macOS 11.0+ |

The first three targets run unmodified in the `cross-compile` job
of `.github/workflows/ci.yml`. Apple targets are cross-compiled on
Linux runners via `osxcross`; the matrix entry sets
`continue-on-error: true` so a broken `osxcross` baseline never
gates the rest of CI. Real Apple silicon is covered by
`matrix-os` running on `macos-latest`.

## Linux — glibc vs musl

- The default Ubuntu runner produces a binary linked against
  `glibc 2.39`. It will run on any distribution with `glibc >= 2.31`
  (Debian 11, RHEL 8.4, Ubuntu 20.04, Alpine 3.13 with `glibc` flavor).
- The `*-musl` targets produce a fully static binary that runs on
  every Linux, including scratch containers, distroless images, and
  routers. Pick musl when the deployment target is unknown.
- musl has no `getaddrinfo_a`; DNS resolution is sequential. Not
  relevant for this CLI (single hostname, one request at a time),
  but worth noting if you embed the crate.

## macOS — Intel and Apple Silicon

- The `matrix-os` job runs the full test suite on `macos-latest`
  (currently Apple Silicon) and produces a binary that uses the
  Apple silicon `dyld` and `SecTrust` paths. The Intel binary is
  cross-compiled in `cross-compile` via `osxcross` and is
  published for users on older hardware.
- `reqwest` is configured with `rustls` (no `native-tls`), so
  certificate validation does not depend on the macOS keychain.
  The bundled `webpki-roots` are used for all TLS handshakes.
- No code signing, no notarisation, no `xcrun altool`. Distribution
  is `cargo install` or `brew install daniloaguiarbr/tap/youtube-legend-cli`.

## Windows — MSVC toolchain

- Only the `x86_64-pc-windows-msvc` target is supported. The
  `x86_64-pc-windows-gnu` target is not built and is not in the
  `[package.metadata.docs.rs].targets` list.
- The end user needs the Microsoft Visual C++ 2015-2022
  Redistributable. Most modern Windows 10/11 machines already
  have it. If you ship an installer, the WiX bundle can be a
  prereq.
- `reqwest` uses `rustls` on Windows. No need for `schannel`,
  no need for the Windows certificate store.
- Path handling goes through the `directories` crate; the cache
  lives at `%LOCALAPPDATA%\youtube-legend-cli\cache\`.
- Signal handling for `Ctrl+C` is wired through the same
  `tokio_util::CancellationToken` used on Unix; the `signal_handler_stress`
  integration test exercises both code paths.

## Containers

### Scratch + musl (smallest image)

```dockerfile
FROM rust:1.88-alpine AS builder
RUN apk add --no-cache musl-dev
RUN cargo install youtube-legend-cli --locked --root /out

FROM scratch
COPY --from=builder /out/bin/youtube-legend-cli /youtube-legend-cli
ENTRYPOINT ["/youtube-legend-cli"]
```

Resulting image: ~12 MB. No shell, no libc, no package manager.

### Distroless + glibc

```dockerfile
FROM rust:1.88-slim AS builder
RUN cargo install youtube-legend-cli --locked --root /out

FROM gcr.io/distroless/cc-debian12
COPY --from=builder /out/bin/youtube-legend-cli /usr/local/bin/
ENTRYPOINT ["youtube-legend-cli"]
```

Resulting image: ~30 MB. Includes `glibc`, `libgcc`, `libstdc++`,
no shell.

### Alpine

```dockerfile
FROM rust:1.88-alpine AS builder
RUN apk add --no-cache musl-dev
RUN cargo install youtube-legend-cli --locked --features headless --root /out

FROM alpine:3.20
RUN apk add --no-cache chromium
COPY --from=builder /out/bin/youtube-legend-cli /usr/local/bin/
ENTRYPOINT ["youtube-legend-cli"]
```

The `headless` feature is opt-in. When enabled, the binary
expects a `chromium` or `chrome` binary reachable via `$CHROME`
or at the well-known system paths.

## Shell Support

The CLI is a pure stdin/stdout contract; shell is the orchestration
layer. All five major shells are exercised in the docs link-check
and example build matrix:

- `bash` 4+ on Linux/macOS, 5+ on Windows via Git Bash or WSL.
- `zsh` 5+ (default on modern macOS).
- `fish` 3+.
- `elvish` 0.18+ (smoke-tested via `examples/batch`).
- `powershell` 7+ on Windows and PowerShell Core on Linux/macOS.

Completion scripts can be generated with `clap`-driven
`--help` introspection; the README ships a copy-pasteable block
for each shell.

## File Paths and XDG

- Linux: `$XDG_CACHE_HOME/youtube-legend-cli/cache/` (default
  `~/.cache/youtube-legend-cli/cache/`). Config file at
  `$XDG_CONFIG_HOME/youtube-legend-cli/config.toml`.
- macOS: `~/Library/Caches/youtube-legend-cli/cache/`. Config at
  `~/Library/Application Support/youtube-legend-cli/config.toml`.
- Windows: `%LOCALAPPDATA%\youtube-legend-cli\cache\`. Config at
  `%APPDATA%\youtube-legend-cli\config.toml`.
- The cache is TTL-keyed, default 24 hours (`--cache-ttl <HOURS>`).
  Use `--no-cache` to skip reads; use `--cache-ttl 0` to disable
  writes only.

The path is resolved via the `directories` crate, which honours
`XDG_CACHE_HOME` and `HOME` on Unix and `LOCALAPPDATA` on Windows.

## Performance by Target

Wall-clock for a single `cargo build --release` of the CLI on
the CI hardware (cache warm, 4 cores):

| Target | Build time | Binary size |
|---|---|---|
| `x86_64-unknown-linux-gnu` | ~3 min | 8.4 MB stripped |
| `x86_64-unknown-linux-musl` | ~3 min 10 s | 8.5 MB stripped |
| `aarch64-unknown-linux-musl` | ~6 min (QEMU) | 8.3 MB stripped |
| `x86_64-pc-windows-msvc` | ~4 min | 8.7 MB stripped |
| `x86_64-apple-darwin` | `continue-on-error` | ~9.1 MB |
| `aarch64-apple-darwin` | `continue-on-error` | ~9.0 MB |

Strip + `lto = "thin"` is enabled in the release profile
(`Cargo.toml` `[profile.release]`). The CI `Verify binary size`
step fails the build if the binary exceeds 20 MB (NFR-003).

## Agents Validated per Platform

| Agent | Linux | macOS | Windows | Notes |
|---|---|---|---|---|
| `claude -p` (Claude Code) | Yes | Yes | Yes | OAuth-only since v0.x of the runner |
| `codex exec` (OpenAI Codex) | Yes | Yes | Yes | Requires `codex` CLI on `PATH` |
| Aider | Yes | Yes | Yes | Uses the `--json` envelope for streaming |
| Continue.dev | Yes | Yes | Yes | Plugin supports the `commands` shape |
| Goose | Yes | Yes | Partial | Pipe-friendly via stdin/stdout |
| Shell-only loop | Yes | Yes | Yes (Git Bash) | No agent needed; pure `curl`-style |

## See Also

- [README](../README.md) — install, flags, exit codes.
- [docs/MIGRATION.md](MIGRATION.md) — what changed at v0.2.9 / v0.3.0.
- [docs/TESTING.md](TESTING.md) — how the six targets are exercised in CI.
- [docs/ARCHITECTURE.md](ARCHITECTURE.md) — module map and provider pipeline.
