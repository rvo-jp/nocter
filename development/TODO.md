# Nocter Development Handoff

## Current State

Nocter v0.36.0 is published and externally audited. v0.37.0 Phase 0 and Phase 1 are complete. The
independent baseline and bottleneck attribution identify monolithic whole-program finalization as
the dominant repeated editor cost. No production optimization has been made.

## Next Work

Complete v0.37.0 Phase 2 by replacing duplicated unindexed body-relation inputs with one canonical
checking-owned catalog, then measure the released and candidate command/editor paths before
deciding whether further cold-path work is justified.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
