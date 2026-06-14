# ADR-0009: Cargo.toml ownership in parallel agent teams

- Status: Accepted
- Date: 2026-06-14
- Deciders: tech lead
- Tags: agent-teams, coordination, cargo

## Context

The project adopts an Agent Teams workflow where the tech lead spawns
multiple `general-purpose` teammates in parallel to deliver independent
units of work. Each teammate receives a self-contained prompt and is
expected to make atomic edits through `atomwrite`.

A coordination gap surfaced during the 2026-06-13 audit-fix batch.
Implementer T2 added `use mimalloc::MiMalloc;` to `src/main.rs` and
expected `mimalloc` to resolve as a runtime dependency. Implementer T3
worked on a separate file but did not add `mimalloc = "0.1"` to
`Cargo.toml`. The result was a build break: `mimalloc` was referenced
but undeclared, so the binary failed to compile until the tech lead
applied an inline fix on `Cargo.toml`.

The same class of gap is possible whenever two tasks touch
`Cargo.toml` from independent code paths:

- T4 adds `criterion` as a dev-dependency; T5 edits `src/error.rs` and
  `Cargo.toml` for a feature flag.
- T6 normalises a feature in `[features]`; T7 adds a runtime dep that
  should be optional.

Without ownership, the only synchroniser becomes the tech lead reading
the diff at validation time, which is exactly the manual repair this
workflow was meant to avoid.

## Decision

`Cargo.toml` is a serialised file under the Agent Teams workflow.

- One and only one teammate per session owns `Cargo.toml`. The owner
  task must declare the dependency additions, feature flags, and any
  `[features]` table mutations.
- All other teammates MUST read `Cargo.toml` at task start to confirm
  the dependencies they reference are declared. If a dependency is
  missing, the teammate must NOT add it; instead, the teammate must
  report the gap to the lead and request the `Cargo.toml` task to be
  spawned (or unblocked) first.
- `addBlockedBy` chains that include `Cargo.toml` edits must place the
  `Cargo.toml` task BEFORE every code task that depends on the
  declared dependency.
- The `Cargo.toml` task itself MAY run in parallel with tasks that
  touch unrelated files (such as docs, tests, ADR markdown), because
  the only contention is on the `Cargo.toml` BLAKE3 checksum.
- A teammate that needs to mutate `Cargo.toml` outside its owned task
  MUST treat the mutation as a state drift (exit 82 from `atomwrite`)
  and abort the operation, reporting the conflict to the lead.

## Consequences

Positive:
- Zero `Cargo.toml` build breaks from missing dependencies.
- Cheaper validation: the lead reads one `Cargo.toml` diff per session
  rather than inferring it from N code diffs.
- Clearer audit trail: every dependency addition is tied to one task
  description and one checksum.

Negative:
- Loses some parallelism. A code task that needs a new dep must wait
  for the `Cargo.toml` task to finish. In practice this is at most
  one extra task slot.
- The `Cargo.toml` task becomes a serial bottleneck if many features
  land in one session. Mitigation: the `Cargo.toml` task can be a
  meta-task that runs early and is fully unblocked.

Risk:
- The `Cargo.toml` owner might over-claim: add deps that are not yet
  used. Mitigation: code reviews in the PR catch unused additions.

## Alternatives considered

- Serialise all teammates, never run in parallel. Rejected because
  the bottleneck is real and the workflow exists specifically to
  exploit non-overlapping file edits.
- Use a pre-spawn shared manifest where each task declares its
  required deps. Rejected because the manifest duplicates
  `Cargo.toml` and drifts.
- Trust the lead to fix gaps inline as they did for T2/T3. Rejected
  because the inline fix is silent and bypasses the workflow.

## Follow-up actions

- Update the Agent Teams runbook to include a pre-spawn checklist
  item: "Does any task reference a new dependency? If yes, the
  `Cargo.toml` task is blocked-by every dependent task."
- Add a validation gate: after the parallel phase, the lead runs
  `cargo check --all-targets` BEFORE `cargo build`, because
  `cargo check` surfaces undeclared-dep errors faster than `build`.
- Optional: add an `atomwrite` wrapper that rejects `--workspace`-root
  writes to `Cargo.toml` unless the task ID matches the registered
  owner.
