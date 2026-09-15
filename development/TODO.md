# Nocter Development Handoff

## Current State

Nocter v0.51.0 is published and externally audited. The v0.52.0 Compile-Time Callable Evaluation
candidate is complete and qualified, publication is authorized, and public metadata now selects
v0.52.0. Authored `const` capability reaches one canonical checked-body projection, structural
constants are isolated from final checked initializer values, and `CheckedProgram` publishes closed
values and callable specializations through one `CompileTimeProgram`. The adopted design lives in
[`development/history/milestones/v0.52.0.md`](history/milestones/v0.52.0.md).

## Next Work

Commit this publication metadata, create annotated tag `v0.52.0`, fast-forward `main`, upload the
retained qualified archive as the release's single asset, and verify the public tag, release asset,
latest-release endpoint, downloaded archive, installed identity, and source-identified Pages
deployment. Do not rebuild or replace the qualified archive.

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
