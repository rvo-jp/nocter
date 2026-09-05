# Nocter Development Handoff

## Current State

Nocter v0.36.0 is published and externally audited. v0.37.0 Phase 0 is active and establishes the
measurement authority and released performance baseline before any production optimization.

## Next Work

Complete the v0.36.0 process-cold command and persistent body-edit LSP baseline with the independent
benchmark runner. Then attribute the dominant path using compiler-owned query accounting before
changing production code.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
