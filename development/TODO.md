# Nocter Development Handoff

## Current State

Nocter v0.51.0 is published and externally audited. v0.52.0 Compile-Time Callable Evaluation is in
progress. Phases 0-2 are complete: authored `const` capability reaches one canonical checked-body
projection, and `CheckedProgram` owns the only identity-indexed compile-time plan table. The
adopted design and remaining phases live in
[`development/history/milestones/v0.52.0.md`](history/milestones/v0.52.0.md).

## Next Work

Implement v0.52.0 Phase 3 by separating declared constant identities and types from evaluated
values, then introduce the identity-keyed semantic dependency query described by the design. Do
not expose compile-time calls in public constant syntax until that query owns callable plans and
values. Do not add `isolated`, `deterministic`, `pure`, or `realtime` syntax as part of this
milestone.

Preserve the v0.51.0 tag, release asset, public notes, specification snapshot, and publication audit
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
