# Nocter Development Handoff

## Current State

Nocter v0.67.0 Operational Local Applications is active. Phases 0 through 2 are complete: the
milestone contract is fixed, `Store` ownership is enforced by one target-backed non-waiting lock,
and one insertion-ordered `StoreState` now owns a retained seeded lookup index plus allocation-free
lending traversal. v0.66.0 remains the published and externally audited release boundary.

## Next Work

Implement Phase 3 as one persistence-format boundary: add bounded atomic mutation batches and
deterministic complete checkpoints, give the published v0.66.0 snapshot journal one explicit
version-specific decoder, replay either each complete mutation batch or none of it, and normalize a
legacy journal through the ordinary durable replacement authority. Commit, replay, migration, and
compaction must share one record/version authority rather than branching on guessed bytes.

Preserve the v0.66.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains from v0.66.0.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
