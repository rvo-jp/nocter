# Nocter Development Handoff

## Current State

Nocter v0.49.0 is published and externally audited. v0.50.0 is active as one Local Data and
Asynchronous Streaming milestone. Its completion definition and phased authority replacement live
in [`development/history/milestones/v0.50.0.md`](history/milestones/v0.50.0.md).

## Next Work

Begin v0.50.0 Phase 1 by defining the one canonical `File` ownership and operation contract shared
by executor-safe and explicitly blocking surfaces. Implement the generated Darwin blocking service
through the Phase 0 lifecycle authority before replacing the public blocking-only `File` surface.
Do not let standard source know worker records, queue state, wake transport, or native error
encoding.

Preserve every published tag and asset, including v0.49.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
