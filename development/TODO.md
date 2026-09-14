# Nocter Development Handoff

## Current State

Nocter v0.50.0 is published and externally audited. Local data and asynchronous streaming now
compose through executor-safe filesystem operations, one asynchronous-iteration contract, bounded
stream adapters, and the complete `async-file-report` application. Its completion definition and
phased authority replacement live in
[`development/history/milestones/v0.50.0.md`](history/milestones/v0.50.0.md).

## Next Work

Define the next milestone before implementation. Preserve the v0.50.0 tag, release asset, public
notes, specification snapshot, and publication audit without replacement. Any correction requires
a new version and a newly qualified artifact.

Phase 7 added installed execution of `async-file-report` and semantic editor checks against its
real source. Preserve exact standard-module dependency review, canonical formatting for every
runnable example, bounded application memory, and failure cleanup that leaves no temporary or
failed final output.

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
