# Nocter Development Handoff

## Current State

Nocter v0.43.0 is published and externally audited. v0.44.0 development is active on
`develop-v0.44.0`. Phases 0-3 have replaced the old result-type-driven asynchronous producer model
with an explicit `async` declaration modifier and a separate `future T` structural type throughout
the frontend, semantic pipeline, standard library, examples, and editor presentation.

## Next Work

Complete v0.44.0 Phase 4 qualification and review. Run the disposable complete compiler gate,
standard-library and public-example qualification, documentation checks, and an adversarial audit
for duplicate execution authority, result-shape reclassification, source-text inference, and old
syntax remnants before release preparation begins.

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
