# Nocter Development Handoff

## Current State

Nocter v0.54.0 is published and externally audited. v0.55.0 development is active and adds explicit
owning callable erasure before using that single runtime-dispatch model for practical HTTP routing
and complete applications. The adopted boundary lives in
[`development/design/erased-callable-design.md`](design/erased-callable-design.md), and phase state
lives in [`development/history/milestones/v0.55.0.md`](history/milestones/v0.55.0.md).

## Next Work

Complete the two active boundaries without coupling them: finish Phase 1's consuming-call adapter;
build Phase 5's complete routed application and qualification. Readonly
erasure covers owned captures plus direct, fallible, and deferred results even when the callable
lives in a persistent asynchronous frame. Request targets retain their exact spelling and parsed
segments, while the deterministic router validates ambiguity once and retains unmatched request
ownership for explicit application policy. Application response conveniences compose the existing
request/responder transitions; they add no framing, timeout, or connection-policy authority.

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
