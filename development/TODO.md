# Nocter Development Handoff

## Next Work

Complete v0.36.0 Phase 5 by qualifying formatting, LSP features, installed-home behavior, native
execution, target rejection, generated documentation, and the complete authority boundaries for
wall-clock time, UTC calendar conversion, RFC 3339, and filesystem timestamps.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
