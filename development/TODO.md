# Nocter Development Handoff

## Current State

Nocter v0.42.0 Phase 4 is complete on the development branch. The self-contained `async-http`
example crosses a structured client/server join over loopback, and editor plus installed-standard
tests qualify the new async request and whole-body response contracts.

## Next Work

Implement v0.42.0 Phase 5: review the complete compiler, runtime, HTTP, documentation, and example
change for duplicate state machines, ownership gaps, blocking executor work, stale synchronous-
only wording, and obsolete helpers. Then run release qualification without weakening any gate.

Preserve every published tag and asset, including v0.41.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
