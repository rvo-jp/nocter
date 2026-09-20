# Nocter Development Handoff

## Current State

Nocter v0.62.0 is published and externally audited. v0.63.0 Cryptographic Randomness is
implementation-complete and has entered release preparation. Release inputs now select v0.63.0;
final clean-tree verification and deterministic artifact qualification remain.

## Next Work

Commit the v0.63.0 release-content inputs, rerun the complete disposable compiler gate, and run
clean-tree double-generation qualification. Record the exact retained candidate before publishing
the explicitly authorized release.

Preserve the v0.62.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains for v0.63.0 release qualification.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
