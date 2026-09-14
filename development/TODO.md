# Nocter Development Handoff

## Current State

Nocter v0.50.0 is published and externally audited. v0.51.0 Unified Execution Contracts is active.
It separates authored promises, invocation facts, deferred-drive facts, and destruction facts under
one checking-owned relation authority. Its completion definition and phases live in
[`development/history/milestones/v0.51.0.md`](history/milestones/v0.51.0.md).

## Next Work

Complete v0.51.0 Phase 5 with a repository-wide residue and architecture review, complete compiler
verification, generated documentation verification, and fresh installed-toolchain qualification.
Close the milestone only if no duplicate execution authority, temporal-scope inference, stale
effect product, or caller-correctness precondition remains. Do not add `realtime` syntax in this
milestone.

Preserve the v0.50.0 tag, release asset, public notes, specification snapshot, and publication audit
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
