# Nocter Development Handoff

## Current State

Nocter v0.58.0 is published and externally audited. v0.59.0 is qualified for publication from
release-content commit `1a551b8f69e041b5d9675c16d47fbdda1d93dcaf`. Two independent optimized builds
produced the retained 9,372,190-byte archive with SHA-256
`bbb329c6756215a0a2d04b97f971e722dbe8b528273638363d3668d77dab14f7`; the complete compiler,
installed-home, binary-record, editor, immutability, and tamper gates passed. The completed
implementation scope lives in
[`development/history/milestones/v0.59.0.md`](history/milestones/v0.59.0.md).

## Next Work

Await an explicit publication request. Publication must tag the qualified release-content commit,
reuse `dist/nocter-v0.59.0-arm64-darwin.tar.gz` without rebuilding it, publish the single retained
asset and release notes, update current public version references, deploy source-identified Pages,
and record a post-publication audit.

Preserve the v0.58.0 tag, release asset, public notes, specification snapshot, and publication audit
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
