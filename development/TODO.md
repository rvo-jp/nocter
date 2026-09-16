# Nocter Development Handoff

## Current State

Nocter v0.54.0 is published and externally audited. v0.55.0 development is active and adds explicit
owning callable erasure before using that single runtime-dispatch model for practical HTTP routing
and complete applications. The adopted boundary lives in
[`development/design/erased-callable-design.md`](design/erased-callable-design.md), and phase state
lives in [`development/history/milestones/v0.55.0.md`](history/milestones/v0.55.0.md).

## Next Work

Complete the two active boundaries without coupling them: finish Phase 1's consuming-call adapter;
build Phase 3's deterministic HTTP router over the native-qualified readonly erased-call path.
Readonly erasure now covers owned captures plus direct, fallible, and deferred results even when
the callable lives in a persistent asynchronous frame. Request targets retain their exact spelling,
freeze the path/query boundary once, and expose decoded query iteration without a second target
parser.

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
