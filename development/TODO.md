# Nocter Development Handoff

## Current State

Nocter v0.39.0 is published and externally audited. v0.40.0 Phases 0-1 are complete. The active
release remains one synchronous Internet client foundation spanning trusted target services, URLs,
system name resolution, HTTP/1.1 framing, and an owned synchronous client lifecycle.

## Next Work

Begin Phase 2 with system name resolution and host connection policy in `std/net`. Bind only the
Phase 0 trusted resolver services in the Darwin adapter, copy native results into logical
`SocketAddress` values, and release every native result exactly once. Preserve operating-system
candidate order, remove exact duplicates, bypass the resolver for numeric hosts, and share one
monotonic deadline across candidate connection attempts. Keep URL parsing and HTTP framing outside
the resolver boundary. Preserve the immutable v0.39.0 tag and asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
