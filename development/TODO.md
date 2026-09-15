# Nocter Development Handoff

## Current State

Nocter v0.52.0 is published and externally audited. Compile-Time Callable Evaluation lets authored
`const` capability reach one canonical checked-body projection, isolates structural constants from
final checked initializer values, and publishes closed values and callable specializations through
one `CompileTimeProgram`. The adopted design lives in
[`development/history/milestones/v0.52.0.md`](history/milestones/v0.52.0.md).

## Next Work

Plan the next practical milestone from concrete standard-library and application needs. Evaluate
which remaining compile-time restrictions block real APIs before expanding the operation set or
adding another surface capability. Keep one checked semantic authority and do not introduce a
second evaluator, source interpreter, or compile-time-only callable model.

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
