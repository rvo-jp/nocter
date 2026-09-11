# Nocter Development Handoff

## Current State

Nocter v0.45.0 is published and externally audited. Development has moved to v0.46.0 on
`develop-v0.46.0`. The asynchronous datagram area is implemented through its public surface,
native IPv4/IPv6 execution, cancellation and timeout behavior, complete public example, and
ordinary checked LSP queries without weakening the universal `future T` drive invariant.

## Next Work

Perform the whole-area review and release-readiness work defined by the
[v0.46.0 milestone](history/milestones/v0.46.0.md) and
[asynchronous datagram design](design/asynchronous-datagram-io-design.md). Audit generic-syscall
reachability, retry and timeout authority, dependency direction, effect preservation, caller-trust
contracts, and stale documentation. Then run the complete compiler, documentation, public-example,
formatting, and repository gates and stop before changing release identity or publishing.

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
