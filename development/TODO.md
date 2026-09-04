# Nocter Development Handoff

## Current State

Nocter v0.34.0 is [published and externally audited](releases/v0.34.0.md). Its tag and release asset
are immutable.

The current implementation is the published v0.34.0 Unicode-scalar foundation plus the complete
v0.35.0 immutable-static product. Its active scope belongs to the
[v0.35.0 milestone](milestones/v0.35.0.md),
[Phase 0 design review](reviews/v0.35.0-phase-0.md), and
[Phase 1 implementation review](reviews/v0.35.0-phase-1.md).

## Next Work

Implement v0.35.0 Phase 2 as one reproducible Unicode 17.0.0 data generator and one package-private
lookup contract over generated immutable-static tables. Do not add Unicode-specific compiler
primitives or generated control-flow trees. Do not alter the immutable v0.34.0 tag or release
asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
