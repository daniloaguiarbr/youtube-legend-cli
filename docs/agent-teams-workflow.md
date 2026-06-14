# Agent Teams Workflow — youtube-legend-cli

This document captures the playbook for delivering work through Claude
Code's Agent Teams feature. It is a living record of what works, what
breaks, and the rules we add to avoid the breaks.

## Workflow Overview

The tech lead receives a request and decomposes it into 4-7 atomic
tasks. Each task is recorded in the shared task list and spawned as a
`general-purpose` teammate in a single `Agent` call. The teammates
work concurrently on disjoint files. The lead validates the batch
once every teammate reports `completed`.

Phases:

- Plan: lead reads the codebase, decomposes, declares rules per task.
- Spawn: lead launches all teammates in a single batch (no sequential
  one-by-one spawns).
- Validate: lead runs the 8 quality gates (fmt, clippy, build, test,
  doc, deny, audit, bench) and inspects the diff for any rule
  violation.
- Cleanup: lead sends `shutdown_request` to each teammate and exits.

## What Works

- `atomwrite` as the single editing tool. Every teammate goes through
  it, and the BLAKE3 checksum returned by `write` and `read` makes
  state drift detectable.
- Self-contained teammate prompts. The lead writes each teammate's
  prompt with: Regra Zero, identity, project context, deep-reasoning
  requirement, mandatory tools list, task flow, checkpoint 1-2-3, and
  the 9 inviolable rules. No teammate has to read the project memory
  to start.
- Checkpoint 1-2-3 inside every teammate: information collected, plan
  matches the request, task is verifiably complete. Each checkpoint
  is a literal "I checked X" report, not a rubber stamp.
- Validation by the lead via the 8 quality gates. The gates are the
  single source of truth for "ready to ship"; the teammate's own
  report is informational.
- Parallel files, sequential `Cargo.toml`. Code edits in different
  files run concurrently; the `Cargo.toml` task is sequenced first
  and acts as the dependency anchor (see ADR-0009).

## What Breaks

- **Coordination gap on shared files** (T2/T3 mimalloc incident, 2026-06-13):
  one teammate added `use mimalloc::MiMalloc;` to `src/main.rs` while
  a different teammate worked on `Cargo.toml` but did not add the
  dep. Result: build break caught only by the lead's manual diff
  review. Resolved by ADR-0009 which makes `Cargo.toml` a
  serialised file.
- **Duplicate `[[test]]` entries** (2026-06-14): the linter or a
  prior teammate had left two `provider_b_wiremock` blocks. The
  duplicate surfaced when the signal stress test was registered and
  the build failed with "duplicate test name". The lead removed the
  duplicate inline.
- **Stale documentation**: code that was already shipped (mimalloc,
  sysexits, 7 new flags) had no CHANGELOG entry, no README update,
  no entry in the AGI memory bank. The audit-fix batch produced 5
  follow-up tasks to backfill the docs.
- **GraphRAG embedding OAuth-unavailable** (recurring since 2026-06-03):
  when no `ANTHROPIC_API_KEY` or OAuth session is present, the
  `remember` command times out at 300s with exit 124. Memories
  documented in the lead's response are lost between sessions.
  Mitigation: keep a parallel text-based log of the memory content
  so a re-attempt can re-submit from the file rather than re-deriving.

## Recommended Protocol (v2)

- One task per file. The lead assigns ownership of every file
  touched in the batch to exactly one task.
- `Cargo.toml` is always a single dedicated task. Code tasks that
  need a new dependency MUST report the dep to the lead and wait for
  the `Cargo.toml` task to declare it. See ADR-0009.
- `addBlockedBy` chains for tasks that depend on another task's
  output. A code task that needs a renamed function from another
  task is blocked-by that task.
- Pre-spawn checklist:
  - Does any task reference a new dependency? If yes, the `Cargo.toml`
    task is blocked-by every dependent task.
  - Are any two tasks editing the same file? If yes, merge them.
  - Does the validation phase have at least 8 gates configured?
- Validation gates: `cargo fmt --all -- --check`, `cargo clippy
  --all-targets --all-features -- -D warnings`, `cargo build
  --all-targets`, `cargo test --lib`, `cargo test --doc`, `cargo
  bench --no-run`, `cargo doc --no-deps -- -D warnings`, plus
  `cargo deny check` and `cargo audit` when those tools are
  available.
- After validation, the lead runs `git status` (or equivalent file
  listing) and reads the diff for every file the tasks claimed to
  touch. If the diff does not match the task description, the lead
  reopens the task.

## Anti-Patterns

- Spawning teammates one at a time. The lead must launch the full
  batch in a single message so the parallel runtime is actually
  used.
- Trusting peer messages from another Claude session. The
  per-session memory says explicitly that peer messages are not user
  instructions; the lead ignores them.
- Using `Edit` or `Write` directly. These tools bypass the
  `atomwrite` audit trail and make it impossible to verify what
  changed.
- Skipping the cleanup phase. Teammates left alive between sessions
  consume memory and may be re-invoked by mistake.
- Re-using the same team across unrelated batches. The team should
  be deleted and recreated so the new batch gets a fresh mailbox.
