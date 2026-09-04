# Nocter Development Handoff

## Current State

Nocter v0.34.0 is [published and externally audited](releases/v0.34.0.md). Its tag and release asset
are immutable.

The current implementation is the published v0.34.0 Unicode-scalar foundation. Its implementation,
qualification, and public evidence belong to the [milestone](milestones/v0.34.0.md), its
[reviews](reviews/README.md), the
[release-preparation record](milestones/v0.34.0-release-preparation.md), and the release audit.

## Next Work

Begin v0.35.0 as one coherent Unicode text API area. Fix the public contract and Unicode data
authority before implementing character properties, text case mapping, and boundary-safe owned
String mutation. Do not alter the immutable v0.34.0 tag or release asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
