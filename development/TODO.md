# Nocter Development Handoff

## Current State

Nocter v0.56.0 is published and externally audited. v0.57.0 Phases 0–5 are complete: declarations,
checked operations, compile-time plans, executable identities, MIR, layout, ABI lowering, native
execution, editor projection, repeat-array construction, and fixed-capacity standard APIs preserve
one ordered type-or-`usize` application without source reinterpretation or hidden runtime
parameters. The milestone contract lives in
[`development/history/milestones/v0.57.0.md`](history/milestones/v0.57.0.md).

## Next Work

Prepare v0.57.0 for release: perform the release audit from the completed milestone, freeze the
public specification and standard-library surface, then build and verify the distributable without
changing the implemented contract.

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
