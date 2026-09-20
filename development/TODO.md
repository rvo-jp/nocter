# Nocter Development Handoff

## Current State

Nocter v0.62.0 is published and externally audited. v0.63.0 Cryptographic Randomness is
implementation-complete, fully verified, and deterministically qualified. The exact retained
archive is ready for the explicitly authorized publication.

## Next Work

Commit the v0.63.0 qualification evidence, update publication metadata, and publish the exact
retained archive without rebuilding it. Audit the remote tag, release asset, latest endpoint,
downloaded installed home, workflows, and source-identified Pages deployment afterward.

Preserve the v0.62.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains for v0.63.0 publication.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
