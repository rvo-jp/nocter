# Nocter Development Handoff

## Current State

Nocter v0.65.0 Practical Stateful Services is active. Phase 0 owns the shared-allocation and
readiness-notification foundations required by every later service API. The previous v0.64.0
Application Encoding and Identity release is published and externally audited.

## Next Work

Complete Phase 0 without creating a second scheduler model: generic descriptor primitives belong to
the target OS adapter, `std/internal/notify` owns task-notification descriptors, and
`std/internal/shared` owns page-backed shared storage. Then implement bounded coordination before
HTTP or service-lifecycle adoption.

Preserve the v0.64.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No external blocker is known. Phase 0 must prove that shared handles cannot inherit region storage
and that notification cleanup remains correct under future cancellation before the public sync API
is admitted.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
