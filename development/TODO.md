# Nocter Development Handoff

## Next Work

Prepare v0.36.0 for release. Freeze the release identity, run the complete release gate, assemble
and independently qualify the installed home and archive, write the public release note, and stop
before tagging or publishing unless publication is explicitly requested.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
