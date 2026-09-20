# Nocter Development Handoff

## Current State

Nocter v0.65.0 Practical Stateful Services is active. Phases 0 through 4 completed page-backed
shared ownership, descriptor notification, mutexes, bounded channels, cooperative cancellation,
structured service ownership, process-wide termination observation, and bounded HTTP application
data plus explicitly stateful routing. The previous v0.64.0 Application Encoding and Identity
release is published and externally audited.

## Next Work

Complete Phase 5 as one sessions-and-observability layer. Build finite session identifiers from
existing randomness and codec contracts, define explicit transport and redaction boundaries, and
add structured operational records that compose with the stateful router without hidden process
state.

Preserve the v0.64.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No external blocker is known. Phase 5 must choose one session transport contract and one
structured-event ownership model before the complete service depends on either.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
