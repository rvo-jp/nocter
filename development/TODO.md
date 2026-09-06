# Nocter Development Handoff

## Current State

Nocter v0.38.0 is active. Phase 0 defines the complete floating-point semantic and representation
contract. Released behavior remains v0.37.0 until the new behavior is implemented, qualified, and
published.

## Next Work

Continue Phase 1 from `development/history/milestones/v0.38.0.md`: complete target-owned
compile-time floating arithmetic and the corrected derived-comparison model. Decimal conversion,
lossless numeric conversions, typed-bit transport through Machine, independent ARM64 floating
register allocation, mixed ABI calls, spills, returns, aggregate fields, and generic memory
primitives are complete and natively qualified. Do not substitute host arithmetic or duplicate
Machine's value-class decision in targets, and do not publish partial source behavior.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
