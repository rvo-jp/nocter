# Nocter Development Handoff

## Current State

Nocter v0.63.0 Cryptographic Randomness is published and externally audited. The exact v0.64.0
Application Encoding and Identity candidate from release-content commit
`a5944bbea8c6ed7ba691a78df46b2dae77e4edb1` passed the complete compiler gate and deterministic
artifact qualification. Public metadata now selects v0.64.0; publication and external audit remain.

## Next Work

Publish annotated tag `v0.64.0` and the exact retained qualified archive, then audit the public tag,
single release asset, downloaded installation, Actions runs, and source-identified Pages deployment.

Preserve the v0.63.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains for v0.64.0 publication.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
