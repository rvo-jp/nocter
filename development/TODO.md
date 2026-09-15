# Nocter Development Handoff

## Current State

Nocter v0.53.0 is published and externally audited. v0.54.0 Phases 0–1 are complete. Client
responses and server requests use one canonical body cursor; incoming request bodies implement
ordinary asynchronous reader contracts, consuming finalization drains unread input before
producing the unique responder, and safe transport overread remains owned for later reuse.

## Next Work

Implement Phase 2 as one coherent response-output change. Make validated response metadata the
shared response-head authority, freeze one fixed-length or chunked response plan before output,
stream bytes through ordinary writer contracts with transport backpressure, and make the existing
owned-body response drive that same writer transition. Do not retain the current complete-response
encoder as an independent framing implementation.

Preserve the v0.52.0 tag, release asset, public notes, specification snapshot, and publication audit
without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
