# Nocter Development Handoff

## Current State

Nocter v0.49.0 is published and externally audited. v0.50.0 is active as one Local Data and
Asynchronous Streaming milestone. Its completion definition and phased authority replacement live
in [`development/history/milestones/v0.50.0.md`](history/milestones/v0.50.0.md).

## Next Work

Continue v0.50.0 Phase 1 by defining the operation-job frame ABI and compiler-owned primitive roles,
then connect them to the generated Darwin file-service root. The root now materializes its fixed
queues, worker group, close-on-exec nonblocking notification pipe, and retirement records directly
from the runtime ABI; native qualification proves repeated ensure and shutdown behavior. The
generated boundary also has bounded capacity, closed operation/access/seek vocabularies, a
validated two-word failure ABI, a typed Darwin import catalog, an opaque compiler-owned
`FileOwner` source binding, atomic job/retirement/service transitions, and pre-reserved close
records that double as explicit-close future frames. The generated wait projection coalesces shared
native registration keys and fans readiness back out to every semantic waiter. Complete the target
boundary before replacing the public blocking-only `File` surface. Standard source must not know
worker records, queue state, wake transport, or native error encoding.

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
