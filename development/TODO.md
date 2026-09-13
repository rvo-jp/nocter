# Nocter Development Handoff

## Current State

Nocter v0.49.0 is published and externally audited. Release-content commit
`63ce5e0c09175f0488ef03fbbf43131b848e8f12` passed the complete compiler, documentation,
reproducible-package, and installed-home gates. Publication commit
`cd2d6e7385fab4eb42153bb6a6c3df40d5033ec4` is the peeled target of annotated tag `v0.49.0`.
The public release is latest, contains exactly one asset, and the downloaded asset matches the
retained qualified archive byte for byte. GitHub Pages now deploys an Actions artifact whose
manifest identifies that same publication commit.

## Next Work

Define the next milestone before changing implementation. Preserve the v0.49.0 child ownership,
endpoint transfer, exact observation, cancellation, abandonment, generic byte-I/O, and Pages
artifact boundaries unless a future public design explicitly replaces them.

Preserve every published tag and asset, including v0.49.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
