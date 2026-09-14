# Nocter Development Handoff

## Current State

Nocter v0.51.0 is published and externally audited. Unified Execution Contracts separates authored
promises, invocation facts, deferred-drive facts, and destruction facts under one checking-owned
relation authority. Its completion definition and phases live in
[`development/history/milestones/v0.51.0.md`](history/milestones/v0.51.0.md).

## Next Work

Plan the next practical milestone before adding another surface guarantee. Audit concrete standard-
library and application needs for compile-time evaluation and explicit ambient authority, then
choose one coherent v0.52.0 scope. Do not add `isolated`, `deterministic`, `pure`, `const func`, or
`realtime` syntax until its exact observable contract and first real consumers are identified.

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
