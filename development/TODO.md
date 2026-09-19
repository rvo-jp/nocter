# Nocter Development Handoff

## Current State

Nocter v0.60.0 is published and externally audited. All three v0.61.0 implementation phases are
complete: repository operability, comment-preserving formatting, and verified local artifact
installation. Release inputs now select v0.61.0; final clean-tree verification and artifact
qualification remain.

## Next Work

Commit the v0.61.0 release-content inputs, rerun the complete gate, and run clean-tree
double-generation qualification, which now includes verified fresh and replacement installation.
Record the exact retained candidate before requesting publication authorization. Do not treat
release checksums or manifest content digests as authenticity guarantees.

Preserve the v0.60.0 tag, release asset, public notes, specification snapshot, and publication audit
without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains for v0.60.0.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
