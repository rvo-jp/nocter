# Nocter Development Handoff

## Current State

Nocter v0.59.0 is published and externally audited. v0.60.0 is qualified for publication from
release-content commit `cc2129e7dd6008bcf0a2f1d177abcf179ea9f1f0`. Two independent optimized
builds produced the retained 9,390,722-byte archive with SHA-256
`5acc99481d923671cef2ca3d7a2ad6322276674eb3239f8e773a4931b514c392`; the complete compiler,
installed-home, archive-inspection, editor, immutability, and tamper gates passed. The completed
implementation scope is recorded in
[`v0.60.0: Streaming Compression and Safe Archives`](history/milestones/v0.60.0.md).

## Next Work

Await an explicit publication request. Publication must tag the qualified release-content commit,
reuse `dist/nocter-v0.60.0-arm64-darwin.tar.gz` without rebuilding it, publish the single retained
asset and release notes, update current public version references, deploy source-identified Pages,
and record a post-publication audit.

Preserve the v0.59.0 tag, release asset, public notes, specification snapshot, and publication audit
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
