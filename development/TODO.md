# Nocter Development Handoff

## Current State

Nocter v0.51.0 is published and externally audited. v0.52.0 Compile-Time Callable Evaluation is
implementation-complete and its release identity is prepared for qualification. Authored `const`
capability reaches one canonical checked-body projection, structural constants are isolated from
final checked initializer values, and `CheckedProgram` publishes closed values and callable
specializations through one `CompileTimeProgram`. The adopted design and completed phases live in
[`development/history/milestones/v0.52.0.md`](history/milestones/v0.52.0.md).

## Next Work

Prepare and qualify the v0.52.0 release candidate. Advance the single packaging version and the
standard-library package version together, write public release notes, generate documentation out
of tree, and commit the complete release-content identity. Then run deterministic packaging twice
and the fresh installed-toolchain matrix. Record exact artifact evidence without rebuilding the
candidate. Do not tag, push, upload, or deploy until publication is explicitly requested.

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
