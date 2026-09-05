# Nocter Development Handoff

## Next Work

Complete v0.36.0 Phase 0 by freezing the `SystemTime`, `UtcDateTime`, RFC 3339, target wall-clock,
and filesystem timestamp contracts. Then implement the target fact and normalized instant before
calendar or text conversion so later layers cannot reinterpret raw clock values.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
