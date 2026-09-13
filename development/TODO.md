# Nocter Development Handoff

## Current State

Nocter v0.49.0 is published and externally audited. v0.50.0 is active as one Local Data and
Asynchronous Streaming milestone. Its completion definition and phased authority replacement live
in [`development/history/milestones/v0.50.0.md`](history/milestones/v0.50.0.md).

## Next Work

Continue v0.50.0 Phase 1 by materializing operation-job construction, bounded admission, worker
execution, and result consumption from the frozen frame ABI, then bind the exact source primitive
roles. The root now materializes its fixed queues, worker group, close-on-exec nonblocking
notification pipe, and retirement records directly from the runtime ABI. Generated retirement is
complete from reservation through owner publication, drop or explicit close, cancellation,
validated failure consumption, record reuse, and group drain; native qualification transfers and
closes a real descriptor through that path. The operation frame separately owns complete inputs,
worker output bytes, consumer-only read destination, explicit scalar operands/results, failure
facts, and its shared wake interest. Complete the target boundary before replacing the public
blocking-only `File` surface. Standard source must not know worker records, queue state, wake
transport, or native error encoding.

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
