# Nocter Development Handoff

## Current State

Nocter v0.44.0 is published and externally audited. Nocter v0.45.0 is qualified for publication
from release-content commit `03f80fa57ff9843a760742e0f7ac128473719ccb`. Public latest-release
references remain at v0.44.0 until the publication commit.
The standard library has nonwaiting network disposal, provider-backed asynchronous host setup, an
explicit synchronous resolver effect, exact primitive blocking validation, propagated synchronous
contracts, single-layer awaited failures, and canonical base names for asynchronous TCP, TLS, and
HTTP operations. Synchronous twins consistently end in `_blocking`. The adopted model makes
nonblocking drive a universal `future T` invariant and marks synchronous external waiting with the
positive `blocking` callable effect.

## Next Work

Publish v0.45.0 using only the retained qualified
`dist/nocter-v0.45.0-arm64-darwin.tar.gz` archive, then download and compare the public asset and
record the immutable publication audit. The implementation and qualification contracts are in the
[v0.45.0 milestone](history/milestones/v0.45.0.md) and
[release preparation record](history/milestones/v0.45.0-release-preparation.md).

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
