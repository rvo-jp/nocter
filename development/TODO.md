# Nocter Development Handoff

## Current State

Nocter v0.45.0 is published and externally audited. Development has moved to v0.46.0 on
`develop-v0.46.0`. The next release completes asynchronous UDP without weakening the universal
`future T` drive invariant or inferring nonblocking behavior from generic syscall arguments.

## Next Work

Implement the closed datagram target operations defined by the
[v0.46.0 milestone](history/milestones/v0.46.0.md) and
[asynchronous datagram design](design/asynchronous-datagram-io-design.md). Then build one
cancellation-safe descriptor substrate, migrate the public UDP surface, and qualify native and
editor behavior before release review.

Preserve every published tag and asset, including v0.45.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
