# Nocter Development Handoff

## Current State

Nocter v0.67.0 Operational Local Applications is active. Phases 0 through 5 are complete: the
milestone contract is fixed; `Store` ownership is enforced by one target-backed non-waiting lock;
one insertion-ordered `StoreState` owns lookup and traversal; journal version `2` publishes bounded
mutation batches with deterministic checkpoint fallback while explicitly normalizing the published
v0.66.0 format; and `std/config` owns source-neutral schemas, failure-atomic overlays, validation,
provenance, immutable results, and secret-safe presentation. Scalar JSON objects, explicitly
selected process environment variables, structured command-line results, and authored defaults now
enter that same source vocabulary without adapter-owned precedence or validation. v0.66.0 remains
the published and externally audited release boundary.

## Next Work

Implement Phase 6 as one reloadable service lifecycle. Add a distinct reload observation, Darwin
`SIGHUP` integration, candidate construction and validation outside the published state, atomic
publication only after success, and retention of the prior valid configuration after a failed
reload. Keep signal observation separate from configuration reading and forbid process-global
mutable configuration.

Preserve the v0.66.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains from v0.66.0.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
