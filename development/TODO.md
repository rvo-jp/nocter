# Nocter Development Handoff

## Current State

Nocter v0.67.0 Operational Local Applications is active. Phases 0 through 7 are complete: the
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
v0.66.0 remains the published and externally audited release boundary.

## Next Work

Implement Phase 8 qualification and review. Run cross-process contention, journal crash-tail and
migration, reload race, installed-home example, compiler, LSP, native, documentation, deterministic
packaging, and responsibility-boundary gates. Review for duplicate authority, repeated
interpretation, reverse dependencies, and contracts that require callers to preserve hidden
invariants. Keep application policy in the example rather than adding a service/configuration
convenience wrapper.

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
