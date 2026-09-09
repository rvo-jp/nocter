# Nocter Development Handoff

## Current State

Nocter v0.42.0 Phase 0 is complete on the development branch. HTTP request normalization and final
response-head selection have one transport-independent authority consumed by the synchronous
client. The async HTTP ownership and immediate/deferred boundary is documented.

## Next Work

Implement v0.42.0 Phase 1: async request transmission and response-body consumption over the
existing async TCP surface. Synchronous name resolution must finish before returning the lazy
computation, and sync and async paths must share protocol policy and the unique response state.

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
