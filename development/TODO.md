# Nocter Development Handoff

## Next Work

Complete v0.36.0 Phase 4 by exposing filesystem modification timestamps through the implemented
`SystemTime` authority and adding one runnable integration example. Then qualify tooling and the
full release surface without duplicating calendar arithmetic or target ABI knowledge.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
