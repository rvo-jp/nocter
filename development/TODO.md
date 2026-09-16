# Nocter Development Handoff

## Current State

Nocter v0.53.0 is published and externally audited. v0.54.0 Phases 0–4 are complete. Incoming
requests and client responses share one canonical body cursor. Server output freezes one validated
fixed-length or chunked plan and streams through a linear `ResponseWriter`; successful completion
returns the same connection only when the selected HTTP/1.1 policy permits reuse. Graceful shutdown
stops listener admission before one deadline-bounded drain of the application-owned handler group.

## Next Work

Implement Phase 5 as one coherent application and qualification change. Extend the public service
example to transfer a body larger than every fixed transport buffer over a reused connection while
retaining bounded admission, backpressure, forced close, malformed framing, and graceful shutdown.
Then qualify compiler, editor, standard library, native execution, documentation, installation,
and packaging together and perform the whole-repository review. Do not create parallel example-only
protocol or lifecycle paths.

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
