# Nocter Development Handoff

## Current State

Nocter v0.44.0 is published and externally audited. v0.45.0 Phases 0 through 3 are complete and
Phase 4 is next.
The standard library has nonwaiting network disposal, provider-backed asynchronous host setup, an
explicit synchronous resolver effect, exact primitive blocking validation, propagated synchronous
contracts, single-layer awaited failures, and canonical base names for asynchronous TCP, TLS, and
HTTP operations. Synchronous twins consistently end in `_blocking`. The adopted model makes
nonblocking drive a universal `future T` invariant and marks synchronous external waiting with the
positive `blocking` callable effect.

## Next Work

Complete Phase 4 by qualifying semantic tooling and practical asynchronous integration over the
closed effect and structured-task contracts. Follow the phases and gates in the
[v0.45.0 milestone](history/milestones/v0.45.0.md).

Preserve every published tag and asset, including v0.44.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
