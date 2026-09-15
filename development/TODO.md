# Nocter Development Handoff

## Current State

Nocter v0.51.0 is published and externally audited. The v0.52.0 Compile-Time Callable Evaluation
candidate is implementation-complete and qualified. Authored `const` capability reaches one
canonical checked-body projection, structural constants are isolated from final checked
initializer values, and `CheckedProgram` publishes closed values and callable specializations
through one `CompileTimeProgram`. The adopted design and completed phases live in
[`development/history/milestones/v0.52.0.md`](history/milestones/v0.52.0.md).

## Next Work

Publish v0.52.0 only when explicitly requested. Reuse the retained qualified archive without
rebuilding it, create the annotated tag and public release metadata, update current-version entry
points and generated documentation, deploy the source-identified Pages artifact, and perform the
post-publication download and installation audit. Record immutable publication evidence afterward.

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
