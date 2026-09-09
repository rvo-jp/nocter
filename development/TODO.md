# Nocter Development Handoff

## Current State

Nocter v0.42.0 Phase 3 is complete on the development branch. Practical request constructors and
text mutation derive from validated request values. Async whole-body byte and UTF-8 collection
consume the existing unique response cursor and delegate timeout behavior to the established
per-input idle-timeout operation.

## Next Work

Implement v0.42.0 Phase 4: add a complete asynchronous local HTTP example, editor coverage, public
documentation, and packaged qualification. External Internet access must remain unnecessary for
tests and examples must not imply HTTPS, redirect, retry, or connection-reuse support.

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
