# Nocter Development Handoff

## Current State

Nocter v0.43.0 is published and externally audited. v0.44.0 development is active on
`develop-v0.44.0`. Phase 0 has replaced the old result-type-driven asynchronous producer contract
with an explicit `async` declaration modifier and a separate `future T` structural type.

## Next Work

Implement v0.44.0 Phase 1 across syntax, formatting, semantic types, declaration lowering,
validation, and semantic presentation. Delete the `async T` parser and result-shape classification;
do not add a compatibility parser or migration fallback.

Preserve every published tag and asset, including v0.43.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
