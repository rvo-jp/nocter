# Nocter Development Handoff

## Current State

Nocter v0.60.0 is published and externally audited. All three v0.61.0 implementation phases are
complete: repository operability, comment-preserving formatting, and verified local artifact
installation. Phase 2 still requires the complete disposable compiler gate and clean-tree release
qualification before v0.61.0 release preparation begins.

## Next Work

Run the complete v0.61.0 verification and artifact-install qualification, then perform release
preparation as a separate audited change. Do not treat release checksums or manifest content
digests as authenticity guarantees.

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
