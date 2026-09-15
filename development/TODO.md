# Nocter Development Handoff

## Current State

Nocter v0.51.0 is published and externally audited. v0.52.0 Compile-Time Callable Evaluation is in
progress. Phases 0-3 are complete: authored `const` capability reaches one canonical checked-body
projection, header values and closed callable specializations are resolved by typed dependency
queries, and `CheckedProgram` publishes both immutable strata through one `CompileTimeProgram`.
The
adopted design and remaining phases live in
[`development/history/milestones/v0.52.0.md`](history/milestones/v0.52.0.md).

## Next Work

Continue v0.52.0 Phase 4 from the implemented closed-plan executor and the standard library's first
real scalar-policy consumers. Admit direct function and method calls in constant and static
initializer plans by feeding ordinary checking's selected call identity and typed arguments into
that executor; do not teach the header expression planner to perform overload or generic
selection. The remaining architectural work must separate structural constant dependencies needed
to finish declaration types from ordinary initializer values completed after checking; it must not
publish optional declaration values or run a second body checker. Extend initializer values through
tuples and fixed arrays without creating a source-reading evaluator. Do not add `isolated`,
`deterministic`, `pure`, or `realtime` syntax as part of this milestone.

Preserve the v0.51.0 tag, release asset, public notes, specification snapshot, and publication audit
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
