# Nocter Development Handoff

## Current State

Nocter v0.36.0 is published and externally audited. v0.37.0 Phase 0 through Phase 2 are complete.
The independent baseline and bottleneck attribution identify monolithic whole-program finalization
as the dominant repeated editor cost. The shared body-relation catalog removes repeated input
construction and lookup, but measured editor latency improved by only 1.4 percent per edit.

## Next Work

Complete v0.37.0 Phase 3 by moving source-neutral relation facts and body-local finalization
products across the body query boundary. Preserve canonical type and closure identity, and retain
one shared semantic path for commands and editor features.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
