# Nocter Development Handoff

## Current State

Nocter v0.65.0 Practical Stateful Services is implementation-complete and has entered release
preparation. Release inputs now select v0.65.0; final clean-tree verification and deterministic
artifact qualification remain. The previous v0.64.0 Application Encoding and Identity release is
published and externally audited.

## Next Work

Commit the exact v0.65.0 release-content inputs, rerun the complete disposable compiler gate, and
run clean-tree double-generation qualification. Record the retained candidate evidence, then stop
before publication unless the user explicitly authorizes it. Do not rebuild between qualification
and publication.

Preserve the v0.64.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains for v0.65.0 release qualification.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
