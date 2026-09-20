# Nocter Development Handoff

## Current State

Nocter v0.65.0 Practical Stateful Services is active. Phases 0 through 2 completed page-backed
shared ownership, descriptor notification, mutexes, bounded channels, cooperative cancellation,
structured service ownership, and process-wide termination observation. The previous v0.64.0
Application Encoding and Identity release is published and externally audited.

## Next Work

Complete Phase 3 as one HTTP application-data layer: strict query and form decoding, parsed media
types, and cookie parsing and rendering over existing URL, scan, header, and encoding contracts.
Do not introduce HTTP-local percent decoding, token scanners, or general text containers. Preserve
owned request data across suspension and keep every borrowed view tied to its owner.

Preserve the v0.64.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No external blocker is known. Phase 3 must define strict duplicate-key, malformed-escape, media
parameter, cookie-name, and size-bound behavior before stateful routing depends on those values.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
