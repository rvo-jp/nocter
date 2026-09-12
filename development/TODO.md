# Nocter Development Handoff

## Current State

Nocter v0.48.0 is published and externally audited. v0.49.0 Phase 1 is complete on
`develop-v0.49.0`. Descriptor readiness, fixed deadlines, and process completion now share one
semantic wait vocabulary and one Darwin event model in both the host adapter and generated ARM64
process root. Exit-before-registration becomes immediate readiness without reaping status or
periodic probing.

## Next Work

Implement v0.49.0 Phase 2 as one ownership change: add the exact child-owner record and
compiler-owned abandonment target service, keep explicit observation separate from cleanup, and
prove that cancellation or destruction transfers every unobserved child without blocking the
executor or losing its sole reaping obligation.

Preserve every published tag and asset, including v0.48.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
