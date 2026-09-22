# Nocter Development Handoff

## Current State

Nocter v0.67.0 Operational Local Applications is active. Phases 0 through 3 are complete: the
milestone contract is fixed; `Store` ownership is enforced by one target-backed non-waiting lock;
one insertion-ordered `StoreState` owns lookup and traversal; and journal version `2` publishes
bounded mutation batches with deterministic checkpoint fallback while explicitly normalizing the
published v0.66.0 format. v0.66.0 remains the published and externally audited release boundary.

## Next Work

Implement Phase 4 as one source-neutral configuration core. Define explicit field schemas,
source-neutral candidate values, caller-ordered overlay application, typed validation, immutable
finalized configuration, source provenance, unknown and duplicate input rejection, and secret-safe
diagnostics and presentation. The core must not know JSON, process-environment, CLI, or service
lifecycle representations; those adapters belong to later phases.

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
