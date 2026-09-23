# Nocter Development Handoff

## Current State

Nocter v0.67.0 Operational Local Applications is published and externally audited. All eight phases
are complete: the
milestone contract is fixed; `Store` ownership is enforced by one target-backed non-waiting lock;
one insertion-ordered `StoreState` owns lookup and traversal; journal version `2` publishes bounded
mutation batches with deterministic checkpoint fallback while explicitly normalizing the published
v0.66.0 format; and `std/config` owns source-neutral schemas, failure-atomic overlays, validation,
provenance, immutable results, and secret-safe presentation. Scalar JSON objects, explicitly
selected process environment variables, structured command-line results, and authored defaults now
enter that same source vocabulary without adapter-owned precedence or validation. One process-owned
lifecycle source now distinguishes consumable reload from sticky termination, and
`PublishedConfiguration` can expose only a fully validated immutable value while retaining the
previous value after candidate failure. The complete HTTP service now applies those contracts to
bounded concurrent admission, incremental durable events, installed-home reload, rejected-candidate
retention, exclusive store ownership, redacted output, graceful shutdown, and restart recovery.
The annotated tag, single public artifact, downloaded installed home, latest-release endpoint,
Compiler workflow, and Pages deployment all resolve to the recorded v0.67.0 identities.

## Next Work

Define the next milestone as one coherent user-facing capability before implementation begins.
Preserve the v0.67.0 release-content commit, publication tag, retained asset, release notes,
specification snapshot, and audit without replacement. Any correction to a published artifact
requires a new version and a newly qualified archive.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains from v0.67.0.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
