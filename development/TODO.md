# Nocter Development Handoff

## Current State

Nocter v0.44.0 is published and externally audited. v0.45.0 Phases 0 and 1 are complete. Phase 2
has established nonwaiting network disposal, provider-backed asynchronous host setup, an explicit
synchronous resolver effect, exact primitive blocking validation, and propagated synchronous
standard-library contracts. The adopted model makes nonblocking drive a universal `future T`
invariant and marks synchronous external waiting with the positive `blocking` callable effect.

## Next Work

Finish Phase 2 API normalization from the closed effect inventory, then begin structured task
composition in Phase 3. Follow the phases and gates in the [v0.45.0
milestone](history/milestones/v0.45.0.md).

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
