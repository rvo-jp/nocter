# Nocter Development Handoff

## Current State

Nocter v0.34.0 is [published and externally audited](releases/v0.34.0.md). Its tag and release asset
are immutable.

The current implementation is the published v0.34.0 Unicode-scalar foundation plus the complete
v0.35.0 immutable-static and generated Unicode-data products. Its active scope belongs to the
[v0.35.0 milestone](milestones/v0.35.0.md),
[Phase 0 design review](reviews/v0.35.0-phase-0.md), and
[Phase 1 implementation review](reviews/v0.35.0-phase-1.md), and
[Phase 2 implementation review](reviews/v0.35.0-phase-2.md).

## Next Work

Implement v0.35.0 Phase 3 character properties and borrowed Unicode whitespace trimming through
the existing `std/internal/unicode` contract. Promote only lookup functions that acquire an actual
cross-module consumer. Keep ASCII APIs direct and unchanged, and do not alter the immutable
v0.34.0 tag or release asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
