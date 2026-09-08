# Nocter Development Handoff

## Current State

Nocter v0.39.0 is published and externally audited. The exact v0.40.0 release candidate remains
qualified and retained locally. v0.41.0 implementation and source-tree qualification are complete
on `develop-v0.41.0`, with no open practical finding. Public latest-release references remain at
v0.39.0, and no v0.41.0 release identity has been assigned.

## Next Work

Wait for explicit authorization before entering v0.41.0 release preparation. That operation must
first resolve release sequencing with the retained unpublished v0.40.0 candidate, then assign the
v0.41.0 identity, update release metadata and public latest-release references, build one
reproducible archive, qualify installed-home execution from that exact archive, and record the
release audit. Publication remains a separate explicitly authorized operation.

Do not rebuild, publish, or otherwise mutate the retained v0.40.0 candidate as an incidental part
of v0.41.0 work. Preserve the immutable v0.39.0 tag and asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
