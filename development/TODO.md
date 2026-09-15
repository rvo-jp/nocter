# Nocter Development Handoff

## Current State

Nocter v0.53.0 development is active. Phases 0 through 4 are complete: JSON compact generation uses
one effect-neutral pull encoder, `TaskGroup<T>` provides runtime-sized structured ownership, and
the HTTP module now exposes a bounded one-request server typestate without detached work. The
active scope and completion gates live in
[`development/history/milestones/v0.53.0.md`](history/milestones/v0.53.0.md).

## Next Work

Implement Phase 5 as one complete bounded service using only public APIs. Use `TaskGroup` as the
connection owner, enforce an application-selected outstanding-task limit, exercise successful and
failing handler outcomes, and qualify editor presentation, installed packaging, and the complete
repository. Do not add an implicit server loop or hidden task registry.

Preserve the v0.52.0 tag, release asset, public notes, specification snapshot, and publication audit
without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
