# Nocter Development Handoff

## Current State

Nocter v0.53.0 is published and externally audited. v0.54.0 Phases 0–2 are complete. Incoming
requests and client responses share one canonical body cursor. Server output freezes one validated
fixed-length or chunked plan and streams through a linear `ResponseWriter`; complete owned
responses drive the same writer instead of retaining a second encoder.

## Next Work

Implement Phase 3 as one coherent persistence change. Select connection disposition once while
validating the request, generate matching response policy from the frozen plan, and return a
`ServerConnection?` only after request and response completion prove reuse safe. Retained overread
must transfer intact; pipelined bytes may be parsed only after the prior response finishes.

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
