# Nocter Development Handoff

## Current State

Nocter v0.55.0 is published and externally audited. v0.56.0 is active and generalizes `from` into
one value-provenance contract for lending and zero-copy APIs. The active scope and completion gates
live in [`development/history/milestones/v0.56.0.md`](history/milestones/v0.56.0.md).

## Next Work

Complete Phase 0 by adopting the unified value-contract specification and proving that its
declaration model can represent result, parameter, receiver, and local constraints without source
order or source spellings. Then replace the result-only compiler path in Phase 1.

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
