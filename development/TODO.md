# Nocter Development Handoff

## Current State

Nocter v0.59.0 is published and externally audited. The corrected v0.60.0 replacement candidate is
qualified after release-candidate review found complete decoded-byte accounting and an ARM64
large-copy address-lifetime defect. The earlier candidate and its digest are superseded and must
not be published. The completed implementation scope is recorded in
[`v0.60.0: Streaming Compression and Safe Archives`](history/milestones/v0.60.0.md).

## Next Work

Publish only the retained qualified `dist/nocter-v0.60.0-arm64-darwin.tar.gz` from release-content
commit `290960a0959769bb000f315f15271aff84463d33`. Do not rebuild it. After publication, verify the
remote tag, release asset digest and size, archive layout, installed manifest, and fresh installed
workflows before recording the publication audit.

Preserve the v0.59.0 tag, release asset, public notes, specification snapshot, and publication audit
without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No implementation or qualification blocker remains. Publication still requires explicit user
authorization.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
