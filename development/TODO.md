# Nocter Development Handoff

## Current State

Nocter v0.51.0 is published and externally audited. v0.52.0 Compile-Time Callable Evaluation is in
progress. Phases 0-4 are complete: authored `const` capability reaches one canonical checked-body
projection, structural constants are isolated from final checked initializer values, and
`CheckedProgram` publishes closed values and callable specializations through one
`CompileTimeProgram`. The
adopted design and remaining phases live in
[`development/history/milestones/v0.52.0.md`](history/milestones/v0.52.0.md).

## Next Work

Complete v0.52.0 Phase 5 review and qualification. Review the structural-constant and checked
initializer strata for duplicate evaluation, source reinterpretation, stale adapters, and values
that can be paired with the wrong declaration/type authority. Add a real standard-library
initializer that calls an existing scalar `const` helper if doing so improves the source rather
than creating a demonstration-only API. Then run compiler, LSP, formatter, standard-library,
installed-toolchain, native, documentation, and packaging gates. Do not add `isolated`,
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
