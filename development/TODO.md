# Nocter Development Handoff

## Current State

Nocter v0.65.0 Practical Stateful Services is active. Phases 0 and 1 completed the page-backed
shared-ownership, descriptor-notification, mutex, bounded-channel, and cooperative-cancellation
foundation. The previous v0.64.0 Application Encoding and Identity release is published and
externally audited.

## Next Work

Complete Phase 2 by giving services one termination-observation and structured shutdown path over
the existing cancellation and task contracts. Do not add a second scheduler, detached task owner,
timer-polling loop, or process-specific state to `std/sync`. Then carry that lifecycle into HTTP
application-data and stateful-routing phases.

Preserve the v0.64.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No external blocker is known. The service-lifecycle design must keep signal ownership target-bound,
while the public shutdown contract remains target-independent and cooperatively observable.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
