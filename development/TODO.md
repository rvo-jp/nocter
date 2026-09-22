# Nocter Development Handoff

## Current State

Nocter v0.67.0 Operational Local Applications is active. Phases 0 through 4 are complete: the
milestone contract is fixed; `Store` ownership is enforced by one target-backed non-waiting lock;
one insertion-ordered `StoreState` owns lookup and traversal; journal version `2` publishes bounded
mutation batches with deterministic checkpoint fallback while explicitly normalizing the published
v0.66.0 format; and `std/config` owns source-neutral schemas, failure-atomic overlays, validation,
provenance, immutable results, and secret-safe presentation. v0.66.0 remains the published and
externally audited release boundary.

## Next Work

Implement Phase 5 as adapters over the source-neutral configuration contract. JSON objects, process
environment snapshots, structured CLI arguments, and authored defaults must produce ordinary
`Source` values without gaining access to `Builder` internals or duplicating validation, precedence,
provenance, and redaction policy. Preserve caller-authored application order and define exact source
name and field-name mapping at each adapter boundary.

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
