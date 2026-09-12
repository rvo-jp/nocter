# Nocter Development Handoff

## Current State

Nocter v0.48.0 is published and externally audited. Release-content commit
`50ceaf9e3e215cb899ef1a572421bae6a881baa6` passed the complete compiler, documentation,
reproducible-package, and installed-home gates. Publication commit
`34f095e16af90558d2a33e23b6cd188b00c8f18d` is the peeled target of annotated tag `v0.48.0`.
The public release is latest, contains exactly one asset, and the downloaded asset matches the
retained qualified archive byte for byte.

## Next Work

Define the next milestone before changing implementation. Preserve the v0.48.0 generic buffered
I/O ownership, cancellation, terminal-state, and transport-independence contracts unless a future
public design explicitly replaces them.

Preserve every published tag and asset, including v0.48.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
