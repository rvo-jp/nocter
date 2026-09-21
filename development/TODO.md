# Nocter Development Handoff

## Current State

Nocter v0.65.0 Practical Stateful Services is published and externally audited. v0.66.0 Durable
Local Application State has completed implementation, review, the complete compiler gate, and
deterministic artifact qualification. Its release-content commit is
`e3f499b5d38d87073fde1c06e1c474772a1c7752`.

## Next Work

Publish `dist/nocter-v0.66.0-arm64-darwin.tar.gz` with SHA-256
`d5476913c5bf8033f6b9e2b6844a42615b166a3af98b02e95af34043539cf87c` without rebuilding it. Then
verify the annotated tag, single GitHub Release asset, latest-release endpoint, downloaded
installed home, push-triggered workflows, and source-identified Pages deployment in a release
audit.

Preserve the v0.65.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No qualification blocker remains for v0.66.0 publication.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
