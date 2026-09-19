# Nocter Development Handoff

## Current State

Nocter v0.60.0 is published and externally audited. All three v0.61.0 implementation phases are
complete: repository operability, comment-preserving formatting, and verified local artifact
installation. The exact v0.61.0 release-content commit passed the complete gate and formal
double-generation qualification. Its retained candidate is ready for publication.

## Next Work

Request explicit publication authorization. After authorization, publish the exact retained
candidate without rebuilding it, then audit the annotated tag, GitHub Release asset,
latest-release endpoint, downloaded installed home, and source-identified Pages deployment. Do not
treat release checksums or manifest content digests as authenticity guarantees.

Preserve the v0.60.0 tag, release asset, public notes, specification snapshot, and publication audit
without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

Publication authorization is the only remaining v0.61.0 blocker. No implementation or
qualification blocker remains.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
