# Nocter Development Handoff

## Current State

Nocter v0.53.0 development is active. Phase 0 and Phase 1 are complete: JSON compact generation now
uses one effect-neutral pull encoder, String generation is nonblocking, and BlockingWriter output is
an explicit buffered driver over the same chunk sequence. The active scope and completion gates live
in [`development/history/milestones/v0.53.0.md`](history/milestones/v0.53.0.md).

## Next Work

Implement Phase 2 as one dynamic structured-task area derived from HTTP server concurrency needs.
Define ownership, completion, cancellation, and destruction before choosing convenience API names.
Do not introduce detached tasks, a scheduler-specific public representation, or a second future
execution authority.

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
