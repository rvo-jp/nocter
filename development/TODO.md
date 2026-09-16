# Nocter Development Handoff

## Current State

Nocter v0.55.0 is published and externally audited. v0.56.0 Phases 0–2 are complete: `from` now
uses one value-provenance contract across results, inputs, receivers, annotated locals, structural
callables, calls, closures, and editor presentation. The active scope and completion gates live in
[`development/history/milestones/v0.56.0.md`](history/milestones/v0.56.0.md).

## Next Work

Begin Phase 3 by defining lending iterator contracts and proving that a yielded item remains tied
to the active receiver loan. Keep ordinary owning `Iterator` behavior intact; lending is a separate
contract rather than a source-compatible reinterpretation.

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
