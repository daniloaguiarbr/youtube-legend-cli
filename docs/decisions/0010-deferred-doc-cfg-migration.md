# ADR-0010: Deferred doc_cfg migration and baseline rules rust docs

- Status: Accepted
- Date: 2026-06-14
- Deciders: tech lead
- Tags: rustdoc, docs.rs, migration, lint, baseline

## Context

The `rules rust` documentation pack mandates a fully migrated
`doc_auto_cfg → doc_cfg` setup on every CLI published to crates.io
since October 2025. The v0.2.8 audit (memory `yt-legend-v0-2-8-audit-2026-06-14`)
flagged 12 gaps in the project relative to that pack.

The original v0.2.9 plan tried to land the migration in a single
patch-level release. The implementation revealed two blocking
issues:

1. `#![cfg_attr(doc, feature(doc_cfg))]` requires a nightly
   toolchain at the consumer side. The crate's `rust-version =
   "1.88.0"` declares stable as the contract, so any `#[feature(...)]`
   on the crate root breaks stable consumers.
2. The wrapper crate `doc_cfg = "0.1"` was tried as a stable
   replacement. Its macro `doc_cfg::doc_cfg` adds a synthetic
   `cfg(unstable-doc-cfg)` that triggers the `unexpected_cfgs` lint
   on stable, and `--cfg doc` is not yet a no-op in stable builds.
   `#[allow(unexpected_cfgs)]` only suppresses nightly-side warnings
   and does not solve the stable build path.

## Decision

The v0.2.9 release defers the visual `#[doc(cfg(feature = "..."))]`
markers. The `headless` feature remains gated by `#[cfg(feature =
"headless")]` (which compiles cleanly on stable) and is documented
in `README.md`, `CHANGELOG.md`, and `llms-full.txt` (section
"Architecture").

The clippy lints (`doc_markdown`, `missing_errors_doc`,
`missing_panics_doc`, `missing_safety_doc`,
`undocumented_unsafe_blocks`) and the 12 official rustdoc lints
were moved to a centralised `[lints.*]` table in `Cargo.toml`. The
`#![warn/deny(...)]` block at the top of `src/lib.rs` was removed
to avoid the "lint level overridden by Cargo.toml" warning.

`#[doc(keyword = "...")]` was tried and reverted because the
attribute is E0658 unstable; SEO is now served by the existing
`#[doc(alias = "...")]` set, expanded with three more aliases per
item in `Cli`, `AppError`, and `Provider`.

## Consequences

Positive
- v0.2.9 ships clean on `cargo check`, `cargo clippy --all-features
  -- -D warnings`, `cargo doc --all-features`, `cargo test
  --all-features`, and `cargo publish --dry-run`.
- The `headless` feature still compiles on stable 1.88.0 without
  nightly-only attributes.
- The `doc_cfg` migration is now an explicit, tracked decision in
  MADR form.

Negative
- Items behind a Cargo feature are not visually marked on
  `docs.rs` until a stable solution lands. The README and llms
  files compensate.
- `#[doc(keyword = "...")]` is unavailable on stable, so the
  SEO surface is reduced to aliases only.

Follow-up (target v0.3.0 or whenever the `doc_cfg` crate ships
a no-op-for-stable build path)
- Re-evaluate `doc_cfg = "0.1"` once the `unstable-doc-cfg` cfg is
  removed or hidden behind `cfg(doc)`.
- Re-enable `#![warn(rustdoc::missing_doc_code_examples)]` once
  the lint stabilises.
- Add `cargo public-api` snapshot (`api-public-api.txt`) and
  `cargo semver-checks` to CI.
