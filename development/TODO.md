# Nocter Development Handoff

## Current State

Nocter v0.48.0 is published and externally audited. v0.49.0 Phase 3 is complete on
`develop-v0.49.0`. Public `Stdio`, `ProcessIo`, `Child`, and pipe endpoint values have one shared
ownership model with canonical executor-safe `spawn` and explicit `spawn_blocking`. Descriptor
transfer is exact-once, untaken endpoints close before observation, and child/endpoint destruction
remains cleanup-safe.

## Next Work

Begin v0.49.0 Phase 4 by designing generic transfer and lifecycle operations over the Phase 3
`Child` and endpoint contracts. Preserve one child owner, one endpoint state, and the shared
reactor; do not add process-specific buffering or another polling loop.

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
