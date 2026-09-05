# Nocter Development Handoff

## Current State

Nocter v0.36.0 is published and externally audited. v0.37.0 Phase 0 is complete: the independent
runner and clean v0.36.0 command/editor baseline are recorded. No production optimization has been
made.

## Next Work

Complete v0.37.0 Phase 1 by attributing the shared cold-check cost and persistent body-edit cost.
Use compiler-owned query accounting and ordinary profiling before selecting one structural
production change.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
