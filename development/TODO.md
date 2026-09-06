# Nocter Development Handoff

## Current State

Nocter v0.38.0 is active. Phase 0 defines the complete floating-point semantic and representation
contract. Released behavior remains v0.37.0 until the new behavior is implemented, qualified, and
published.

## Next Work

Continue Phase 1 from `development/history/milestones/v0.38.0.md`: give ARM64 an explicit floating
virtual/physical register bank and lower constants, arithmetic, comparison, calls, spills, and
returns from Machine's existing class decision. Then complete compile-time floating operations,
lossless conversions, and the corrected derived-comparison model. Decimal conversion and typed-bit
transport through Machine are complete. Do not substitute host arithmetic or general registers for
the remaining contracts, and do not publish partial source behavior.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
