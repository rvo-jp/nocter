# Nocter Development Handoff

## Current State

Nocter v0.38.0 is active. Phases 0 through 2 are complete, and Phase 3 application integration is
implemented. Released behavior remains v0.37.0 until the complete candidate is reviewed, packaged,
qualified as an installed home, and published.

## Next Work

Close Phase 3 from `development/history/milestones/v0.38.0.md`. Review the complete floating-point
change for duplicate numeric authorities, host-dependent evaluation, representation recovery below
CheckedProgram, ABI reclassification below Machine, partial/total-order conflation, editor-only
meaning, stale unsupported paths, and compatibility fallbacks. Then qualify reproducible packaging
and a fresh installed home before release preparation. Do not publish partial source behavior.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
