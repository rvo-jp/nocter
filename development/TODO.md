# Nocter Development Handoff

## Current State

Nocter v0.34.0 is [published and externally audited](releases/v0.34.0.md). Its tag and release asset
are immutable.

The current implementation is the published v0.34.0 Unicode-scalar foundation. v0.35.0 Phase 0 has
fixed the candidate static-data and practical Unicode-text contract. Its active scope belongs to the
[v0.35.0 milestone](milestones/v0.35.0.md) and
[Phase 0 review](reviews/v0.35.0-phase-0.md).

## Next Work

Implement v0.35.0 Phase 1 as one immutable-static product from syntax through readonly Mach-O data
and semantic tooling. Do not add Unicode-specific compiler primitives or generated control-flow
tables. Do not alter the immutable v0.34.0 tag or release asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
