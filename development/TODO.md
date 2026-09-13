# Nocter Development Handoff

## Current State

Nocter v0.49.0 is published and externally audited. v0.50.0 is active as one Local Data and
Asynchronous Streaming milestone. Its completion definition and phased authority replacement live
in [`development/history/milestones/v0.50.0.md`](history/milestones/v0.50.0.md).

## Next Work

Continue v0.50.0 Phase 1 by replacing the public blocking-only `File` surface in one migration.
The exact source primitive roles are now bound through opaque `FileOwner` and `FileCompletion`
runtime-storage declarations. The generated target family owns all eight
operation constructors, bounded admission, four-queue dispatch, direct worker syscalls, partial
write progress, cancellation, retirement, read-output transfer, one completion ABI, and exact
capacity release. Native qualification opens and reads a real descriptor through construction,
worker publication, consumption, and explicit close. The worker retains every post-publication
resource before its completion transition and never reads a frame that a consumer may release.
The target emits service shutdown once at the process-root return boundary, after source cleanup
and before the ordinary ABI epilogue. Standard source cannot observe worker records, queue state,
wake transport, or native error encoding.

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
