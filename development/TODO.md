# Nocter Development Handoff

## Current State

Nocter v0.49.0 is published and externally audited. v0.50.0 is active as one Local Data and
Asynchronous Streaming milestone. Its completion definition and phased authority replacement live
in [`development/history/milestones/v0.50.0.md`](history/milestones/v0.50.0.md).

## Next Work

Continue v0.50.0 Phase 1 by translating the host-qualified file-operation and resource-retirement
contracts into generated Darwin helpers and compiler-owned primitive roles. The generated boundary
now has one runtime-owned process-context slot, bounded capacity, closed operation/access/seek
vocabularies, a validated two-word failure ABI, a typed Darwin import catalog, and an opaque
compiler-owned `FileOwner` source binding. The host model already proves retained-owner results,
initialized read prefixes, partial write facts, exact close completion, cancellation cleanup, and
bounded retirement. Next materialize the generated service state machine and its future lifecycle;
complete that target boundary before replacing the public blocking-only `File` surface. Standard
source must not know worker records, queue state, wake transport, or native error encoding.

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
