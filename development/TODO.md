# Nocter Development Handoff

## Current State

Nocter v0.45.0 is published and externally audited. It makes nonblocking drive universal to
`future T`, represents synchronous external waiting with the positive `blocking` effect,
normalizes asynchronous I/O names, and adds deterministic structured race and timeout composition.
The public asset is byte-identical to the retained qualified archive built from release-content
commit `03f80fa57ff9843a760742e0f7ac128473719ccb`.
The standard library has nonwaiting network disposal, provider-backed asynchronous host setup, an
explicit synchronous resolver effect, exact primitive blocking validation, propagated synchronous
contracts, single-layer awaited failures, and canonical base names for asynchronous TCP, TLS, and
HTTP operations. Synchronous twins consistently end in `_blocking`. The adopted model makes
nonblocking drive a universal `future T` invariant and marks synchronous external waiting with the
positive `blocking` callable effect.

## Next Work

Plan the next milestone only when requested. Preserve the separation between universal future
drive safety, positive synchronous `blocking`, `noalloc`, deferred execution, and result
provenance; do not infer one fact from another.

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
