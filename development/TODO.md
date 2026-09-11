# Nocter Development Handoff

## Current State

Publication of the qualified v0.45.0 candidate is authorized and in progress. The retained archive
was built from release-content commit `03f80fa57ff9843a760742e0f7ac128473719ccb`; its identity must
remain unchanged through the public audit.
The standard library has nonwaiting network disposal, provider-backed asynchronous host setup, an
explicit synchronous resolver effect, exact primitive blocking validation, propagated synchronous
contracts, single-layer awaited failures, and canonical base names for asynchronous TCP, TLS, and
HTTP operations. Synchronous twins consistently end in `_blocking`. The adopted model makes
nonblocking drive a universal `future T` invariant and marks synchronous external waiting with the
positive `blocking` callable effect.

## Next Work

Integrate the public latest-release surfaces into `main`, create and push one annotated `v0.45.0`
tag, upload the retained archive as the release's only asset, and verify the public tag,
latest-release endpoint, asset bytes, extracted installation, and remote `main`. Record that
evidence and stop.

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
