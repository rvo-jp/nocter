# Nocter Development Handoff

## Current State

Nocter v0.53.0 is published and externally audited. v0.54.0 Phases 0–3 are complete. Incoming
requests and client responses share one canonical body cursor. Server output freezes one validated
fixed-length or chunked plan and streams through a linear `ResponseWriter`; successful completion
now returns the same connection only when the selected HTTP/1.1 policy permits reuse.

## Next Work

Implement Phase 4 as one coherent graceful-lifecycle change. Keep admission closure separate from
accepted connection ownership, drain application-owned `TaskGroup` handlers under an explicit
deadline, and prove that cancellation or destruction retires every request, responder, writer,
and reusable connection exactly once. Do not add a hidden HTTP task registry or connection pool.

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
