# Nocter Development Handoff

## Current State

Nocter v0.56.0 is published and externally audited. v0.57.0 Phases 0–2 are complete: declarations
and checked operations preserve one ordered type-or-`usize` application, callable inference solves
fixed-array lengths, and construction, interface selection, closures, destruction, copyability,
compile-time plans, and executable specialization retain the same values without source
reinterpretation. Phase 3 is active. The milestone contract lives in
[`development/history/milestones/v0.57.0.md`](history/milestones/v0.57.0.md).

## Next Work

Complete v0.57.0 Phase 3 by proving that evaluated capacities remain part of executable identity,
layout, MIR, ABI, and native execution. Then implement the fixed-capacity standard APIs and editor
qualification without introducing a second evaluator or downstream source interpretation.

Preserve the v0.52.0 tag, release asset, public notes, specification snapshot, and publication audit
without replacement. Any correction requires a new version and a newly qualified artifact.

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
