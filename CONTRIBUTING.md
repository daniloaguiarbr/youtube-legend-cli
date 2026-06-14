# Contributing to youtube-legend-cli

Thanks for your interest in the project. This document explains how to
set up a development environment, run the test suite, and submit a
change.

## Code of Conduct

All contributors are expected to follow the [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Reporting bugs

Open a GitHub issue with a minimal reproduction. Include the
`youtube-legend-cli --version` output and the exact command line
that triggered the bug, with the URL redacted if needed.

## Reporting security vulnerabilities

See [SECURITY.md](SECURITY.md). Do not file public issues for
security-sensitive problems.

## Development environment

- Rust 1.88.0 or newer (CI runs stable, beta, and the pinned toolchain).
- `cargo fmt`, `cargo clippy`, `cargo test`, `cargo bench`, and
  `cargo doc` are the required tools. `mimalloc`, `criterion`,
  `wiremock`, `assert_cmd`, `predicates`, `serial_test`, and
  `libc` are resolved automatically through normal dependency
  resolution.
- The `headless` feature is opt-in:
  `cargo build --features headless`. The `headless` feature pulls
  in `chromiumoxide` and `futures`, and needs a local Chromium/Chrome
  install at runtime.

## Workflow

1. Fork the repository and create a topic branch.
2. Make your change. Add or update tests.
3. Run the eight quality gates before opening a PR:

   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo build --release --all-features
   cargo test --lib --all-features
   cargo test --doc --all-features
   cargo bench --no-run
   cargo doc --no-deps --all-features -- -D warnings
   cargo deny check    # when cargo-deny is on PATH
   cargo audit         # when cargo-audit is on PATH
   ```

4. Open a pull request against `main`.

## Agent Teams workflow

The v0.2.6 release was delivered through Claude Code's Agent Teams
feature. The playbook lives at
[`docs/agent-teams-workflow.md`](docs/agent-teams-workflow.md). The
high-level rules:

- One and only one task per file. Two tasks editing the same file
  are merged before spawn.
- `Cargo.toml` is a serialised file under Agent Teams; see
  [`docs/decisions/0009-cargo-toml-ownership-in-parallel-tasks.md`](docs/decisions/0009-cargo-toml-ownership-in-parallel-tasks.md)
  for the rationale.
- All file mutations go through `atomwrite` so the BLAKE3 checksum
  is captured per write and a state drift (exit 82) aborts the
  operation.
- The validation phase runs the eight quality gates above. A
  teammate's own report is informational; the gates are the source
  of truth.

## Style

- Rust edition 2021, MSRV 1.88.0 (declared in Cargo.toml).
- All public items must have a `///` doc comment.
- All `unsafe` blocks must carry a `// SAFETY:` line that explains
  the invariant being upheld.
- Error messages and `Display` impls are written in English; this
  is a technical CLI consumed by scripts and other tools.
- `cargo fmt` formatting is canonical; do not hand-format.
- The crate is `#![deny(rustdoc::bare_urls)]` and
  `#![deny(rustdoc::invalid_html_tags)]`; avoid raw URLs in doc
  comments and prefer `[text](url)` form.

## Commit messages

- Imperative mood, one-line subject under 72 characters, optional
  body wrapped at 72.
- Reference the relevant issue or PR number when applicable.
- Do not include `Co-authored-by` trailers.

## License

By contributing you agree that your contribution is dual-licensed under
the terms of [LICENSE](LICENSE) (MIT or Apache-2.0, at the maintainer's
option).
