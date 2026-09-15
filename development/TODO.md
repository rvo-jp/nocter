# Nocter Development Handoff

## Current State

Nocter v0.53.0 is published and externally audited. v0.54.0 Phase 0 is complete: the existing HTTP
body and connection paths have been audited, and one body cursor plus a linear
`ServerConnection -> IncomingRequest -> Responder -> ServerConnection?` ownership model is adopted.
The milestone and current cross-boundary design own the exact completion contract.

## Next Work

Implement Phase 1 as one coherent change: extract the transport-independent canonical body cursor,
remove the duplicate client completion flag, make incoming server requests stream through `Reader`
and `TimedReader`, and make consuming body finalization drain unread bytes before transferring the
unique responder. Preserve overread bytes for later sequential reuse; do not keep the collecting
server path as a compatibility implementation.

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
