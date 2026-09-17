# Nocter Development Handoff

## Current State

Nocter v0.56.0 is published and externally audited. v0.57.0 Phase 0 is active and establishes the
single semantic authority for `usize` constant parameters and arguments before fixed-capacity
standard APIs depend on them. The milestone contract lives in
[`development/history/milestones/v0.57.0.md`](history/milestones/v0.57.0.md).

## Next Work

Complete v0.57.0 Phase 0: freeze the grammar and replace type-only generic metadata with one ordered
parameter schema whose constant arguments become normalized semantic terms or evaluated values.
Then proceed through type construction, inference, layout, fixed-capacity APIs, and editor
qualification without introducing a second evaluator or downstream source interpretation.

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
