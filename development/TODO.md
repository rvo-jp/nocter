# Nocter Development Handoff

## Current State

Nocter v0.60.0 is published and externally audited. v0.61.0 repository-operability work is
complete. Comment-preserving formatting is complete and has passed the full compiler, native,
editor, documentation, and reproducibility gate.

## Next Work

Close the verified artifact-installation trust, acquisition, destination, replacement, and
recovery contracts before implementation begins. Do not treat the current checksum manifest as an
authenticity guarantee.

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
