# Nocter Development Handoff

## Current State

Nocter v0.44.0 is published and externally audited. v0.45.0 Phase 0 is complete. The adopted model
makes nonblocking drive a universal `future T` invariant and marks synchronous external waiting
with the positive `blocking` callable effect. It does not introduce `noblock` or a second future
type.

## Next Work

Implement Phase 1 as one syntax-to-checking authority replacement. Establish one checked blocking
effect before changing standard-library API names. Follow the phases and gates in the
[v0.45.0 milestone](history/milestones/v0.45.0.md).

Preserve every published tag and asset, including v0.44.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
