# Nocter Development Handoff

## Current State

Nocter v0.65.0 Practical Stateful Services is published and externally audited. v0.66.0 Durable
Local Application State is active; Phases 0–3 established durable filesystem replacement, the
recoverable journal, the byte-oriented store, and bounded compaction.

## Next Work

Implement v0.66.0 Phase 4: extend the complete HTTP service example with bounded durable
application state, commit only after its structured handler drain, close the store, reopen it, and
verify that the same committed state survives restart. Keep application serialization and
concurrent-task synchronization above the byte-store contract.

Preserve the v0.64.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains from v0.65.0.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
