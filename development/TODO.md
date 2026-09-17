# Nocter Development Handoff

## Current State

Nocter v0.56.0 is published and externally audited. v0.57.0 Phase 0 is complete: syntax preserves
mixed generic arguments, declarations own one ordered type-or-`usize` parameter schema, and fixed
array types retain closed values or symbolic parameter identities without downstream source
interpretation. Phase 1 is active. The milestone contract lives in
[`development/history/milestones/v0.57.0.md`](history/milestones/v0.57.0.md).

## Next Work

Complete v0.57.0 Phase 1 by replacing type-only application payloads with one ordered mixed
application, validating each source argument against its declaration parameter domain, evaluating
closed constant arguments once, and substituting symbolic constant parameters. Then proceed through
inference, layout, fixed-capacity APIs, and editor qualification without introducing a second
evaluator or downstream source interpretation.

Preserve the v0.52.0 tag, release asset, public notes, specification snapshot, and publication audit
without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
