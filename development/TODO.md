# Nocter Development Handoff

## Current State

Nocter v0.38.0 is active. Phase 0 defines the complete floating-point semantic and representation
contract. Released behavior remains v0.37.0 until the new behavior is implemented, qualified, and
published.

## Next Work

Implement Phase 1 from `development/history/milestones/v0.38.0.md`: carry `f32` and `f64` through
the language and native pipeline, including the comparison/total-order separation and an explicit
Machine register class. Do not publish partial source behavior.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
