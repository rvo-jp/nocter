# Nocter Development Handoff

## Current State

Nocter v0.44.0 is published and externally audited. All v0.45.0 implementation phases are complete,
and the release identity is fixed at `0.45.0`. Public latest-release references remain at v0.44.0
until the publication commit.
The standard library has nonwaiting network disposal, provider-backed asynchronous host setup, an
explicit synchronous resolver effect, exact primitive blocking validation, propagated synchronous
contracts, single-layer awaited failures, and canonical base names for asynchronous TCP, TLS, and
HTTP operations. Synchronous twins consistently end in `_blocking`. The adopted model makes
nonblocking drive a universal `future T` invariant and marks synchronous external waiting with the
positive `blocking` callable effect.

## Next Work

Commit the release-content identity, run the complete disposable-target compiler gate, build the
archive twice from isolated targets, qualify the retained installed home, and record its exact
source and artifact identities. Publication is authorized but must reuse only that retained
qualified archive. The implementation and qualification contracts are in the
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
