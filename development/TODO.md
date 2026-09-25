# Nocter Development Handoff

## Current State

Nocter v0.68.0 Production Native Performance is published and externally audited. The completed
v0.69.0 and v0.70.0 implementation work is included in the qualified v0.71.0 Guaranteed Structural
Operations candidate rather than receiving separate public archives. Linux target work remains
deliberately deferred.

## Next Work

Publish and audit the exact qualified v0.71.0 archive without rebuilding it. Publication is
explicitly authorized for this release. After the public boundary is closed, begin v0.72.0 with a
flow-proof model for safe operations; do not combine that new design with the immutable release
identity.

Preserve the v0.68.0 release-content commit, publication tag, retained asset, release notes,
specification snapshot, and audit without replacement. Any correction to a published artifact
requires a new version and a newly qualified archive.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains. `notrap async` is deliberately rejected because `future T` does not retain a
drive-time trap contract.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
