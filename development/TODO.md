# Nocter Development Handoff

## Current State

Nocter v0.65.0 Practical Stateful Services is active. Phases 0 through 5 completed page-backed
shared ownership, descriptor notification, mutexes, bounded channels, cooperative cancellation,
structured service ownership, process-wide termination observation, and bounded HTTP application
data, explicitly stateful routing, opaque session identifiers, fixed HTTP session transport, and
structured redacted events. The previous v0.64.0 Application Encoding and Identity release is
published and externally audited.

## Next Work

Complete Phase 6 as one application and review closure. Exercise startup, concurrent stateful
requests, session transport, operational events, shutdown, and state recovery in the complete
service; then run the full compiler, editor, native, standard-library, documentation, example, and
packaging validation matrix and review authority boundaries before release preparation.

Preserve the v0.64.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No external blocker is known. Phase 6 must not add a second session store, logging sink registry,
or service lifecycle merely to make the complete example convenient.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
