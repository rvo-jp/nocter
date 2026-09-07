# Nocter Development Handoff

## Current State

Nocter v0.39.0 is published and externally audited. The exact v0.40.0 release candidate is qualified
and retained locally. Public latest-release references remain at v0.39.0 until publication is
separately authorized. v0.41.0 planning is isolated on `develop-v0.41.0`; no release identity has
been changed.

## Next Work

Confirm the three async surface choices in the
[v0.41.0 milestone](history/milestones/v0.41.0.md#phase-0-feasibility-and-surface-contract), then
write the normative Phase 0 language contract before changing compiler syntax. Keep the qualified
v0.40.0 archive unchanged.

Publish v0.40.0 only when explicitly requested. Publication must occur from `main`, reuse the
retained qualified archive without rebuilding it, update public latest-release references, create
one annotated tag, push the exact publication commit and tag, upload exactly one asset, and verify
the public download byte for byte. Preserve the immutable v0.39.0 tag and asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
