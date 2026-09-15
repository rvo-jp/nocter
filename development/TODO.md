# Nocter Development Handoff

## Current State

Nocter v0.53.0 development is active. Phases 0 through 3 are complete: JSON compact generation uses
one effect-neutral pull encoder, `TaskGroup<T>` provides runtime-sized structured ownership, and
the HTTP module now exposes a bounded one-request server typestate without detached work. The
active scope and completion gates live in
[`development/history/milestones/v0.53.0.md`](history/milestones/v0.53.0.md).

## Next Work

Implement Phase 4 as operational safety over the completed HTTP server lifecycle. Add explicit
accept, request-read, and response-write timeout operations; verify cleanup on every timeout,
transport failure, codec failure, handler failure, and task cancellation path. Keep connection and
task limits in application-owned orchestration rather than hidden global server state.

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
