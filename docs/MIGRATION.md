[English](MIGRATION.md) | [Português Brasileiro](MIGRATION.pt-BR.md)
# Migration Guide — youtube-legend-cli

> Move from v0.2.x to v0.3.x with zero surprises.

## What Changes

The v0.3.0 release adds a first-class YouTube-direct provider
and three new flags. The default behaviour for users who never
set a flag is preserved: the CLI still talks to the same third-
party providers in the same order. The new flags and the new
provider slot into the existing pipeline as an opt-in tier.

| Area | v0.2.9 | v0.3.0 |
|---|---|---|
| Providers in chain | ProviderA then ProviderB | YouTube-direct then ProviderA then ProviderB |
| `--provider` flag | absent | `auto`, `youtube-direct`, `provider-a`, `provider-b`, `provider-headless` |
| `--asr` flag | absent | `bool`, only valid with `youtube-direct` |
| `--no-fallback` flag | absent | `bool`, only valid with `--provider auto` |
| `--dry-run` behaviour | served from cache | served from cache, also skips YouTube-direct when set |
| Binaries shipped | `youtube-legend-cli`, `snapshot` | `youtube-legend-cli`, `snapshot`, `youtube-direct-probe` |
| JSON envelope | unchanged | unchanged (additive-only fields) |
| Exit codes | BSD `sysexits.h` (64-78) | BSD `sysexits.h` (64-78) |
| MSRV | `1.88.0` | `1.88.0` |

The `Provider` trait and the `ProviderA` / `ProviderB` concrete
implementations are untouched. Embedders that pull this crate
as a library do not need to recompile their code.

## Step-by-Step Migration

1. Update the binary. `cargo install youtube-legend-cli --locked --force`.
2. Verify the install. `youtube-legend-cli --version` reports
   `0.3.0` or newer.
3. Smoke-test the default behaviour. Pipe a known URL through
   `youtube-legend-cli`; the output should be byte-identical to
   v0.2.9 for the same input.
4. Audit your scripts for flag regressions. The 17 flags you
   knew in v0.2.9 are present and behave identically. New flags
   (`--provider`, `--asr`, `--no-fallback`) are additive and do
   not change defaults.
5. Audit your scripts for new capabilities. The `auto` chain
   now starts with YouTube-direct. If you have hard-coded an
   expectation that "the first network call hits provider A",
   pin it explicitly: `--provider provider-a`.
6. Test JSON consumers. The envelope is additive-only; existing
   `jq` filters keep working. New fields under `meta.provider`
   may appear; defensive parsers should ignore unknowns.
7. If you embed the library, link against the new `Provider`
   re-export. The `ProviderYouTubeDirect` struct is reachable
   via `youtube_legend_cli::provider::ProviderYouTubeDirect`.
   It is `pub` but the public API of the trait surface did not
   change.
8. Roll out behind a flag. For fleet-wide deployment, ship
   v0.3.0 with `--provider auto` and watch the metrics. The
   `dry_run` gate on the new tier is a safety net.

## JSON Schema Changes

The `--json` envelope is the same shape as v0.2.9 with
additive fields. A minimal envelope (v0.2.9) looks like:

```json
{
  "url": "https://youtu.be/dQw4w9WgXcQ",
  "video_id": "dQw4w9WgXcQ",
  "language": "en",
  "format": "txt",
  "provider": "provider-a",
  "body": "...",
  "cached": false,
  "elapsed_ms": 1234
}
```

A v0.3.0 envelope with the YouTube-direct tier selected adds:

```json
{
  "url": "...",
  "video_id": "...",
  "language": "en",
  "format": "txt",
  "provider": "youtube-direct",
  "body": "...",
  "cached": false,
  "elapsed_ms": 987,
  "meta": {
    "provider": "youtube-direct",
    "captions_url": "https://www.youtube.com/api/timedtext?...",
    "deciphered_signature": "<redacted>"
  }
}
```

Existing parsers should treat the `meta` block as opaque and
keep using top-level fields. The `deciphered_signature` is
intentionally redacted; consumers that need the raw signature
should call the embedder API directly, not parse the CLI output.

The authoritative schema is at
`docs/schemas/caption-track.schema.json`.

## Compatibility Notes

- BC-BREAK  exit codes: not in v0.3.0. The BSD `sysexits.h`
  mapping was introduced at v0.2.6 and is preserved.
- BC-BREAK  JSON envelope: none. Additive only.
- BC-BREAK  CLI flags: none. The 17 wired flags keep their
  semantics. New flags (`--provider`, `--asr`, `--no-fallback`)
  are pure additions.
- BC-BREAK  library API: none. `Provider` trait, `ProviderA`,
  `ProviderB`, and `ProviderChain` keep their public surface. The
  new `ProviderYouTubeDirect` is additive.
- BC-BREAK  cache layout: none. Cache files are forward
  and backward compatible across v0.2.6 and v0.3.0.
- BC-BREAK  dependencies: `reqwest 0.13` (was 0.12) landed
  at v0.2.6 already; this release does not touch major versions.

## Rollback

If a v0.3.0 rollout misbehaves, roll back to v0.2.9 in three
steps:

1. `cargo install youtube-legend-cli --version 0.2.9 --locked --force`.
2. Restore your previous scripts. The 17 flags you had are
   unchanged; only the new flags stop being recognised.
3. Clear the local cache. v0.3.0 writes a new `meta.provider`
   field that v0.2.9 does not understand; stale cache files are
   read by both versions, but the new field is ignored by
   v0.2.9. No manual cleanup is required.

Pin the version in your scripts with the explicit `--version`
flag at install time. The CLI does not auto-upgrade; the binary
on disk is the binary that runs.

## See Also
- [CHANGELOG.md](../CHANGELOG.md) — full release history.
- [docs/ARCHITECTURE.md](ARCHITECTURE.md) — provider pipeline and
  chain semantics.
- [docs/CROSS_PLATFORM.md](CROSS_PLATFORM.md) — six cross-compile
  targets, container recipes, XDG paths.
- [docs/TESTING.md](TESTING.md) — how the migration is exercised
  in the integration test suite.
