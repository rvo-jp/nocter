# Nocter Development Handoff

## Current State

Nocter v0.56.0 is published and externally audited. v0.57.0 Phases 0–3 are complete: declarations,
checked operations, compile-time plans, executable identities, MIR, layout, ABI lowering, native
execution, and editor projection preserve one ordered type-or-`usize` application without source
reinterpretation or hidden runtime parameters. Phase 4 is active. The milestone contract lives in
[`development/history/milestones/v0.57.0.md`](history/milestones/v0.57.0.md).

## Next Work

Implement v0.57.0 Phase 4 as one fixed-capacity standard-library area: add an allocation-free byte
buffer and generic stack-resident vector with bounded mutation, then add adapters that use existing
iterator contracts. Keep their capacity in the canonical constant-generic identity and do not add
a second evaluator or downstream source interpretation.

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
