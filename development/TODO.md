# Nocter Development Handoff

## Current State

Nocter v0.48.0 is published and externally audited. v0.49.0 Phase 4 is complete on
`develop-v0.49.0`. Generic blocking and executor-safe byte transfer now compose with public process
pipes. Nonwaiting child observation caches one terminal result, graceful and forced termination
retain the same child owner, and consuming wait or destruction remains the sole reaping boundary.

## Next Work

Begin v0.49.0 Phase 5 by rebuilding the closed status and output operations over the shared launch,
endpoint, and observation authorities. Remove the terminal command-I/O session only after its fair
finite-input and simultaneous-output behavior has migrated; do not create another polling loop or
weaken failure precedence.

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
